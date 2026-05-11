use anyhow::{Context, Result, anyhow};
use async_nats::Client as NatsClient;
use futures_util::future::join_all;
use hsb_mock_endpoints::proto::{
    MockMessageRequest, mock_endpoint_service_client::MockEndpointServiceClient,
};
use hsb_mock_endpoints::{EndpointProfile, EndpointRole, run_endpoint};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use ulid::Ulid;

const MESSAGE_COUNT: usize = 100;
const REPORT_PATH: &str = "target/mock-reports/hl7v3-10-system-report.md";
const NATS_DEFAULT_URL: &str = "nats://127.0.0.1:4222";

#[derive(Clone, Copy, Debug, Serialize)]
enum AccessMode {
    Api,
    Grpc,
    Mq,
    Webservice,
}

impl AccessMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Api => "API",
            Self::Grpc => "gRPC",
            Self::Mq => "MQ",
            Self::Webservice => "WebService",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
enum InteractionPattern {
    OneToMany,
    OneToOne,
    ManyToOne,
}

impl InteractionPattern {
    fn as_str(self) -> &'static str {
        match self {
            Self::OneToMany => "1-to-many",
            Self::OneToOne => "1-to-1",
            Self::ManyToOne => "many-to-1",
        }
    }
}

#[derive(Clone, Debug)]
struct SystemDef {
    endpoint_id: u8,
    service_name: &'static str,
    port: u16,
    role: EndpointRole,
    description: &'static str,
}

#[derive(Clone, Debug)]
struct ScenarioPlan {
    name: &'static str,
    sender: u8,
    targets: &'static [u8],
    transport: AccessMode,
    pattern: InteractionPattern,
    messages: usize,
}

#[derive(Clone, Debug)]
struct DeliveryResult {
    target: u8,
    success: bool,
    latency_ms: u128,
    status: String,
    detail: String,
}

#[derive(Clone, Debug, Serialize)]
struct ScenarioResult {
    name: &'static str,
    sender: &'static str,
    transport: &'static str,
    pattern: &'static str,
    logical_messages: usize,
    delivery_attempts: usize,
    success_deliveries: usize,
    failure_deliveries: usize,
    avg_latency_ms: f64,
    max_latency_ms: u128,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct EndpointStats {
    endpoint_id: u8,
    service_name: String,
    role: String,
    total: u64,
    success: u64,
    failure: u64,
    http_requests: u64,
    grpc_requests: u64,
    nats_messages: u64,
}

fn system_defs() -> Vec<SystemDef> {
    vec![
        SystemDef {
            endpoint_id: 0,
            service_name: "ext-sys-00-api-hub",
            port: 9310,
            role: EndpointRole::Producer,
            description: "API 单发多收入口",
        },
        SystemDef {
            endpoint_id: 1,
            service_name: "ext-sys-01-api-clinic",
            port: 9311,
            role: EndpointRole::Consumer,
            description: "API 目标消费系统",
        },
        SystemDef {
            endpoint_id: 2,
            service_name: "ext-sys-02-ris-core",
            port: 9312,
            role: EndpointRole::Hybrid,
            description: "gRPC 与 MQ 汇聚系统",
        },
        SystemDef {
            endpoint_id: 3,
            service_name: "ext-sys-03-lis-edge",
            port: 9313,
            role: EndpointRole::Consumer,
            description: "API 目标消费系统",
        },
        SystemDef {
            endpoint_id: 4,
            service_name: "ext-sys-04-soap-bridge",
            port: 9314,
            role: EndpointRole::Producer,
            description: "WebService 单发多收入口",
        },
        SystemDef {
            endpoint_id: 5,
            service_name: "ext-sys-05-emr",
            port: 9315,
            role: EndpointRole::Consumer,
            description: "WebService 目标消费系统",
        },
        SystemDef {
            endpoint_id: 6,
            service_name: "ext-sys-06-grpc-adapter",
            port: 9316,
            role: EndpointRole::Hybrid,
            description: "gRPC 单发单收目标",
        },
        SystemDef {
            endpoint_id: 7,
            service_name: "ext-sys-07-grpc-billing",
            port: 9317,
            role: EndpointRole::Hybrid,
            description: "gRPC 单发单收目标",
        },
        SystemDef {
            endpoint_id: 8,
            service_name: "ext-sys-08-queue-a",
            port: 9318,
            role: EndpointRole::Producer,
            description: "MQ 汇聚前置",
        },
        SystemDef {
            endpoint_id: 9,
            service_name: "ext-sys-09-queue-b",
            port: 9319,
            role: EndpointRole::Producer,
            description: "MQ 汇聚前置",
        },
    ]
}

fn scenario_plans() -> Vec<ScenarioPlan> {
    vec![
        ScenarioPlan {
            name: "api-1-to-many-sys0",
            sender: 0,
            targets: &[1, 2, 3],
            transport: AccessMode::Api,
            pattern: InteractionPattern::OneToMany,
            messages: MESSAGE_COUNT,
        },
        ScenarioPlan {
            name: "webservice-1-to-many-sys4",
            sender: 4,
            targets: &[5, 6],
            transport: AccessMode::Webservice,
            pattern: InteractionPattern::OneToMany,
            messages: MESSAGE_COUNT,
        },
        ScenarioPlan {
            name: "grpc-1-to-1-sys2",
            sender: 2,
            targets: &[6],
            transport: AccessMode::Grpc,
            pattern: InteractionPattern::OneToOne,
            messages: MESSAGE_COUNT,
        },
        ScenarioPlan {
            name: "grpc-1-to-1-sys3",
            sender: 3,
            targets: &[7],
            transport: AccessMode::Grpc,
            pattern: InteractionPattern::OneToOne,
            messages: MESSAGE_COUNT,
        },
        ScenarioPlan {
            name: "api-1-to-1-sys5",
            sender: 5,
            targets: &[8],
            transport: AccessMode::Api,
            pattern: InteractionPattern::OneToOne,
            messages: MESSAGE_COUNT,
        },
        ScenarioPlan {
            name: "webservice-1-to-1-sys1",
            sender: 1,
            targets: &[9],
            transport: AccessMode::Webservice,
            pattern: InteractionPattern::OneToOne,
            messages: MESSAGE_COUNT,
        },
        ScenarioPlan {
            name: "mq-many-to-1-sys6",
            sender: 6,
            targets: &[0],
            transport: AccessMode::Mq,
            pattern: InteractionPattern::ManyToOne,
            messages: MESSAGE_COUNT,
        },
        ScenarioPlan {
            name: "mq-many-to-1-sys7",
            sender: 7,
            targets: &[0],
            transport: AccessMode::Mq,
            pattern: InteractionPattern::ManyToOne,
            messages: MESSAGE_COUNT,
        },
        ScenarioPlan {
            name: "mq-many-to-1-sys8",
            sender: 8,
            targets: &[2],
            transport: AccessMode::Mq,
            pattern: InteractionPattern::ManyToOne,
            messages: MESSAGE_COUNT,
        },
        ScenarioPlan {
            name: "mq-many-to-1-sys9",
            sender: 9,
            targets: &[2],
            transport: AccessMode::Mq,
            pattern: InteractionPattern::ManyToOne,
            messages: MESSAGE_COUNT,
        },
    ]
}

#[tokio::main]
async fn main() -> Result<()> {
    let systems = system_defs();
    let plans = scenario_plans();
    let nats_url = resolve_nats_url();
    configure_runtime_env(&plans, &nats_url);

    let mut endpoint_handles = Vec::new();
    for system in &systems {
        let profile = EndpointProfile {
            endpoint_id: system.endpoint_id,
            service_name: system.service_name,
            port: system.port,
            role: system.role.clone(),
            description: system.description,
            supported_protocols: &["HL7V3", "API", "gRPC", "MQ", "WebService"],
            scenario_tags: &["hl7v3", "integration-test", "10-system-simulation"],
        };
        endpoint_handles.push(tokio::spawn(async move { run_endpoint(profile).await }));
    }

    let client = Client::builder().timeout(Duration::from_secs(10)).build()?;
    wait_for_readiness(&client, &systems).await?;
    let nats_client = async_nats::connect(nats_url.clone())
        .await
        .with_context(|| format!("failed to connect real NATS broker at {}", nats_url))?;
    let started_at = chrono::Utc::now();
    let test_started = Instant::now();

    let scenario_tasks = plans.iter().cloned().map(|plan| {
        let systems = systems.clone();
        let client = client.clone();
        let nats_client = nats_client.clone();
        tokio::spawn(async move { execute_scenario(plan, systems, client, nats_client).await })
    });
    let scenario_results = join_all(scenario_tasks)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .collect::<Result<Vec<_>>>()?;

    tokio::time::sleep(Duration::from_millis(300)).await;
    let endpoint_stats = fetch_endpoint_stats(&client, &systems).await?;
    let duration = test_started.elapsed();
    let report = build_report(
        &systems,
        &scenario_results,
        &endpoint_stats,
        &nats_url,
        started_at,
        duration,
    );

    tokio::fs::create_dir_all("target/mock-reports")
        .await
        .context("failed to create report directory")?;
    tokio::fs::write(REPORT_PATH, report.as_bytes())
        .await
        .with_context(|| format!("failed to write report to {}", REPORT_PATH))?;

    for handle in endpoint_handles {
        handle.abort();
    }

    println!("report_path={}", REPORT_PATH);
    println!("duration_ms={}", duration.as_millis());
    println!(
        "logical_messages={}",
        scenario_results
            .iter()
            .map(|item| item.logical_messages)
            .sum::<usize>()
    );
    println!(
        "delivery_attempts={}",
        scenario_results
            .iter()
            .map(|item| item.delivery_attempts)
            .sum::<usize>()
    );
    println!(
        "delivery_success={}",
        scenario_results
            .iter()
            .map(|item| item.success_deliveries)
            .sum::<usize>()
    );

    Ok(())
}

async fn execute_scenario(
    plan: ScenarioPlan,
    systems: Vec<SystemDef>,
    client: Client,
    nats_client: NatsClient,
) -> Result<ScenarioResult> {
    let sender = systems
        .iter()
        .find(|item| item.endpoint_id == plan.sender)
        .ok_or_else(|| anyhow!("unknown sender {}", plan.sender))?;
    let mut success_deliveries = 0usize;
    let mut failure_deliveries = 0usize;
    let mut latency_sum = 0u128;
    let mut max_latency = 0u128;

    for message_index in 0..plan.messages {
        let payload_xmls = plan
            .targets
            .iter()
            .map(|target| {
                let target_system = systems
                    .iter()
                    .find(|item| item.endpoint_id == *target)
                    .ok_or_else(|| anyhow!("unknown target {}", target))?;
                Ok::<_, anyhow::Error>(build_hl7v3_payload(
                    sender,
                    target_system,
                    plan.name,
                    message_index,
                ))
            })
            .collect::<Result<Vec<_>>>()?;

        let outcomes = match plan.transport {
            AccessMode::Api | AccessMode::Webservice => {
                join_all(plan.targets.iter().zip(payload_xmls.into_iter()).map(
                    |(target, payload_xml)| {
                        let client = client.clone();
                        let target_system = systems
                            .iter()
                            .find(|item| item.endpoint_id == *target)
                            .cloned();
                        async move {
                            let target_system = target_system
                                .ok_or_else(|| anyhow!("unknown target {}", target))?;
                            send_http_like(
                                &client,
                                sender.endpoint_id,
                                &target_system,
                                plan.name,
                                message_index,
                                plan.transport,
                                payload_xml,
                            )
                            .await
                        }
                    },
                ))
                .await
            }
            AccessMode::Grpc => {
                join_all(plan.targets.iter().zip(payload_xmls.into_iter()).map(
                    |(target, payload_xml)| {
                        let target_system = systems
                            .iter()
                            .find(|item| item.endpoint_id == *target)
                            .cloned();
                        let sender_name = sender.service_name;
                        async move {
                            let target_system = target_system
                                .ok_or_else(|| anyhow!("unknown target {}", target))?;
                            send_grpc_message(
                                sender.endpoint_id,
                                sender_name,
                                &target_system,
                                plan.name,
                                message_index,
                                payload_xml,
                            )
                            .await
                        }
                    },
                ))
                .await
            }
            AccessMode::Mq => {
                join_all(plan.targets.iter().zip(payload_xmls.into_iter()).map(
                    |(target, payload_xml)| {
                        let target_system = systems
                            .iter()
                            .find(|item| item.endpoint_id == *target)
                            .cloned();
                        let nats_client = nats_client.clone();
                        async move {
                            let target_system = target_system
                                .ok_or_else(|| anyhow!("unknown target {}", target))?;
                            send_nats_message(
                                &nats_client,
                                sender.endpoint_id,
                                &target_system,
                                plan.name,
                                message_index,
                                payload_xml,
                            )
                            .await
                        }
                    },
                ))
                .await
            }
        };

        for outcome in outcomes {
            let outcome = outcome?;
            if outcome.success {
                success_deliveries += 1;
            } else {
                failure_deliveries += 1;
            }
            latency_sum += outcome.latency_ms;
            max_latency = max_latency.max(outcome.latency_ms);
        }
    }

    let delivery_attempts = plan.messages * plan.targets.len();
    let avg_latency_ms = if delivery_attempts == 0 {
        0.0
    } else {
        latency_sum as f64 / delivery_attempts as f64
    };

    Ok(ScenarioResult {
        name: plan.name,
        sender: sender.service_name,
        transport: plan.transport.as_str(),
        pattern: plan.pattern.as_str(),
        logical_messages: plan.messages,
        delivery_attempts,
        success_deliveries,
        failure_deliveries,
        avg_latency_ms,
        max_latency_ms: max_latency,
    })
}

async fn send_http_like(
    client: &Client,
    sender_id: u8,
    target: &SystemDef,
    scenario: &'static str,
    message_index: usize,
    transport: AccessMode,
    payload_xml: String,
) -> Result<DeliveryResult> {
    let started = Instant::now();
    let message_id = Ulid::new().to_string();
    let trace_id = Ulid::new().to_string();
    let sender_name = system_name(sender_id)?;

    let response = match transport {
        AccessMode::Api => {
            let payload = json!({
                "message_id": message_id,
                "trace_id": trace_id,
                "protocol": "HL7V3",
                "message_type": "PRPA_IN201301UV02",
                "source": sender_name,
                "target": target.service_name,
                "scenario": scenario,
                "payload_xml": payload_xml,
                "sequence": message_index,
            });
            client
                .post(format!("http://127.0.0.1:{}/api/v1/messages", target.port))
                .header("x-hsb-protocol", "HL7V3")
                .header("x-trace-id", trace_id)
                .json(&payload)
                .send()
                .await?
        }
        AccessMode::Webservice => {
            client
                .post(format!(
                    "http://127.0.0.1:{}/api/v1/messages/raw",
                    target.port
                ))
                .header("content-type", "text/xml; charset=utf-8")
                .header("x-hsb-protocol", "HL7V3")
                .header("x-hsb-message-type", "PRPA_IN201301UV02")
                .header("x-hsb-source", sender_name)
                .header("x-hsb-target", target.service_name)
                .header("x-hsb-scenario", scenario)
                .header("x-trace-id", trace_id)
                .body(wrap_soap_envelope(&payload_xml))
                .send()
                .await?
        }
        AccessMode::Mq => unreachable!(),
        AccessMode::Grpc => unreachable!(),
    };

    let status = response.status();
    let body = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    let latency_ms = started.elapsed().as_millis();
    let accepted = status.is_success()
        && body
            .get("accepted")
            .and_then(Value::as_bool)
            .unwrap_or(false);

    Ok(DeliveryResult {
        target: target.endpoint_id,
        success: accepted,
        latency_ms,
        status: status.to_string(),
        detail: body
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("UNKNOWN")
            .to_string(),
    })
}

async fn send_grpc_message(
    sender_id: u8,
    sender_name: &'static str,
    target: &SystemDef,
    scenario: &'static str,
    message_index: usize,
    payload_xml: String,
) -> Result<DeliveryResult> {
    let started = Instant::now();
    let mut client =
        MockEndpointServiceClient::connect(format!("http://127.0.0.1:{}", target.port))
            .await
            .with_context(|| format!("failed to connect gRPC target {}", target.service_name))?;
    let message_id = Ulid::new().to_string();
    let trace_id = Ulid::new().to_string();
    let mut request = tonic::Request::new(MockMessageRequest {
        message_id,
        protocol: "HL7V3".to_string(),
        message_type: "PRPA_IN201301UV02".to_string(),
        source: sender_name.to_string(),
        target: target.service_name.to_string(),
        scenario: scenario.to_string(),
        content_type: "application/xml".to_string(),
        payload_json: String::new(),
        raw_payload: payload_xml.into_bytes(),
        headers: HashMap::from([
            ("x-hsb-transport".to_string(), "gRPC".to_string()),
            ("x-sequence".to_string(), message_index.to_string()),
        ]),
    });
    request
        .metadata_mut()
        .insert("x-trace-id", trace_id.parse()?);
    let response = client.handle_message(request).await;
    let latency_ms = started.elapsed().as_millis();

    match response {
        Ok(reply) => {
            let reply = reply.into_inner();
            Ok(DeliveryResult {
                target: target.endpoint_id,
                success: reply.accepted,
                latency_ms,
                status: reply.status,
                detail: format!("{}:{}", sender_id, reply.transport),
            })
        }
        Err(error) => Ok(DeliveryResult {
            target: target.endpoint_id,
            success: false,
            latency_ms,
            status: "gRPC error".to_string(),
            detail: error.to_string(),
        }),
    }
}

async fn send_nats_message(
    nats_client: &NatsClient,
    sender_id: u8,
    target: &SystemDef,
    scenario: &'static str,
    message_index: usize,
    payload_xml: String,
) -> Result<DeliveryResult> {
    let started = Instant::now();
    let message_id = Ulid::new().to_string();
    let trace_id = Ulid::new().to_string();
    let sender_name = system_name(sender_id)?;
    let request_payload = json!({
        "message_id": message_id,
        "trace_id": trace_id,
        "protocol": "HL7V3",
        "message_type": "PRPA_IN201301UV02",
        "source": sender_name,
        "target": target.service_name,
        "scenario": scenario,
        "sequence": message_index,
        "payload_xml": payload_xml,
    })
    .to_string();
    let subject = nats_subject(target.endpoint_id);
    let response = nats_client
        .request(subject.clone(), request_payload.into())
        .await
        .with_context(|| {
            format!(
                "failed to request NATS target {} on {}",
                target.service_name, subject
            )
        })?;
    let response_json = serde_json::from_slice::<Value>(&response.payload)
        .context("failed to decode NATS acknowledgement")?;
    let latency_ms = started.elapsed().as_millis();
    let accepted = response_json
        .get("accepted")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Ok(DeliveryResult {
        target: target.endpoint_id,
        success: accepted,
        latency_ms,
        status: response_json
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("UNKNOWN")
            .to_string(),
        detail: format!("NATS:{}", subject),
    })
}

async fn wait_for_readiness(client: &Client, systems: &[SystemDef]) -> Result<()> {
    for system in systems {
        let url = format!("http://127.0.0.1:{}/health", system.port);
        let mut ready = false;
        for _ in 0..60 {
            if let Ok(response) = client.get(&url).send().await {
                if response.status().is_success() {
                    ready = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        if !ready {
            return Err(anyhow!(
                "system {} on port {} did not become ready",
                system.service_name,
                system.port
            ));
        }
    }
    Ok(())
}

async fn fetch_endpoint_stats(
    client: &Client,
    systems: &[SystemDef],
) -> Result<Vec<EndpointStats>> {
    let mut stats = Vec::new();
    for system in systems {
        let snapshot = client
            .get(format!("http://127.0.0.1:{}/api/v1/stats", system.port))
            .send()
            .await
            .with_context(|| format!("failed to fetch stats from {}", system.service_name))?
            .error_for_status()?
            .json::<EndpointStats>()
            .await
            .with_context(|| format!("failed to decode stats for {}", system.service_name))?;
        stats.push(snapshot);
    }
    Ok(stats)
}

fn build_hl7v3_payload(
    sender: &SystemDef,
    target: &SystemDef,
    scenario: &str,
    message_index: usize,
) -> String {
    format!(
        concat!(
            "<PRPA_IN201301UV02 ITSVersion=\"XML_1.0\" xmlns=\"urn:hl7-org:v3\">",
            "<id root=\"{}\" extension=\"{}\"/>",
            "<creationTime value=\"{}\"/>",
            "<interactionId extension=\"PRPA_IN201301UV02\" root=\"2.16.840.1.113883.1.6\"/>",
            "<processingCode code=\"P\"/>",
            "<sender><device><id root=\"{}\"/></device></sender>",
            "<receiver><device><id root=\"{}\"/></device></receiver>",
            "<controlActProcess classCode=\"CACT\" moodCode=\"EVN\">",
            "<subject><registrationEvent classCode=\"REG\" moodCode=\"EVN\">",
            "<subject1><patient classCode=\"PAT\">",
            "<id extension=\"PAT-{}\" root=\"2.16.156.10011.1.12\"/>",
            "<statusCode code=\"active\"/>",
            "</patient></subject1>",
            "</registrationEvent></subject></controlActProcess>",
            "<attentionLine value=\"{}\"/>",
            "</PRPA_IN201301UV02>"
        ),
        Ulid::new(),
        message_index,
        chrono::Utc::now().format("%Y%m%d%H%M%S"),
        sender.service_name,
        target.service_name,
        message_index,
        scenario,
    )
}

fn resolve_nats_url() -> String {
    std::env::var("HSB_NATS_URLS").unwrap_or_else(|_| NATS_DEFAULT_URL.to_string())
}

fn configure_runtime_env(plans: &[ScenarioPlan], nats_url: &str) {
    set_env("HSB_MOCK_ACCEPT_RATE", "1.0");
    set_env("HSB_NATS_URLS", nats_url);

    for endpoint_id in plans
        .iter()
        .filter(|plan| matches!(plan.transport, AccessMode::Mq))
        .flat_map(|plan| plan.targets.iter().copied())
    {
        set_env(
            &format!("HSB_MOCK_ENDPOINT_{}_NATS_SUBJECT", endpoint_id),
            &nats_subject(endpoint_id),
        );
    }
}

fn set_env(key: &str, value: &str) {
    unsafe {
        std::env::set_var(key, value);
    }
}

fn nats_subject(endpoint_id: u8) -> String {
    format!("hsb.mock.hl7v3.endpoint.{}.inbound", endpoint_id)
}

fn wrap_soap_envelope(payload_xml: &str) -> String {
    format!(
        "<soapenv:Envelope xmlns:soapenv=\"http://schemas.xmlsoap.org/soap/envelope/\"><soapenv:Header/><soapenv:Body>{}</soapenv:Body></soapenv:Envelope>",
        payload_xml
    )
}

fn build_report(
    systems: &[SystemDef],
    scenarios: &[ScenarioResult],
    endpoint_stats: &[EndpointStats],
    nats_url: &str,
    started_at: chrono::DateTime<chrono::Utc>,
    duration: Duration,
) -> String {
    let logical_messages = scenarios
        .iter()
        .map(|item| item.logical_messages)
        .sum::<usize>();
    let delivery_attempts = scenarios
        .iter()
        .map(|item| item.delivery_attempts)
        .sum::<usize>();
    let success_deliveries = scenarios
        .iter()
        .map(|item| item.success_deliveries)
        .sum::<usize>();
    let failure_deliveries = scenarios
        .iter()
        .map(|item| item.failure_deliveries)
        .sum::<usize>();

    let topology_rows = systems
        .iter()
        .map(|system| {
            format!(
                "| {} | {} | {} | {} | {} |",
                system.endpoint_id,
                system.service_name,
                system.port,
                system_role(&system.role),
                system.description
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let scenario_rows = scenarios
        .iter()
        .map(|scenario| {
            format!(
                "| {} | {} | {} | {} | {} | {} | {} | {:.2} | {} |",
                scenario.name,
                scenario.sender,
                scenario.transport,
                scenario.pattern,
                scenario.logical_messages,
                scenario.delivery_attempts,
                scenario.success_deliveries,
                scenario.avg_latency_ms,
                scenario.max_latency_ms,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let endpoint_rows = endpoint_stats
        .iter()
        .map(|stats| {
            format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                stats.endpoint_id,
                stats.service_name,
                stats.role,
                stats.total,
                stats.success,
                stats.failure,
                stats.http_requests,
                stats.grpc_requests,
                stats.nats_messages,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        concat!(
            "# HL7 V3 十系统联调测试报告\n\n",
            "## 测试概览\n\n",
            "- 启动时间: {}\n",
            "- 持续时间: {} ms\n",
            "- 系统数量: {}\n",
            "- 逻辑发送总量: {}\n",
            "- 实际投递总量: {}\n",
            "- 成功投递: {}\n",
            "- 失败投递: {}\n",
            "- HL7 V3 说明: 当前仓库已接入原生 HL7 V3 适配器，本次测试使用 HL7 V3 XML 报文执行解析与跨传输接入验证。\n",
            "- MQ 说明: 本次 MQ 使用真实 NATS broker，地址: {}。\n\n",
            "## 拓扑\n\n",
            "| Endpoint ID | Service | Port | Role | Description |\n",
            "| --- | --- | --- | --- | --- |\n",
            "{}\n\n",
            "## 场景结果\n\n",
            "| Scenario | Sender | Transport | Pattern | Logical Messages | Delivery Attempts | Success Deliveries | Avg Latency ms | Max Latency ms |\n",
            "| --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
            "{}\n\n",
            "## 端点接收统计\n\n",
            "| Endpoint ID | Service | Role | Total | Success | Failure | HTTP | gRPC | NATS |\n",
            "| --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
            "{}\n"
        ),
        started_at.to_rfc3339(),
        duration.as_millis(),
        systems.len(),
        logical_messages,
        delivery_attempts,
        success_deliveries,
        failure_deliveries,
        nats_url,
        topology_rows,
        scenario_rows,
        endpoint_rows,
    )
}

fn system_role(role: &EndpointRole) -> &'static str {
    match role {
        EndpointRole::Producer => "producer",
        EndpointRole::Consumer => "consumer",
        EndpointRole::Hybrid => "hybrid",
    }
}

fn system_name(endpoint_id: u8) -> Result<&'static str> {
    system_defs()
        .into_iter()
        .find(|system| system.endpoint_id == endpoint_id)
        .map(|system| system.service_name)
        .ok_or_else(|| anyhow!("unknown system {}", endpoint_id))
}
