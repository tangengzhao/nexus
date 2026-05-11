use anyhow::{Context, Result, anyhow};
use async_nats::Client as NatsClient;
use futures_util::future::join_all;
use hsb_mock_endpoints::proto::{
    MockMessageRequest, mock_endpoint_service_client::MockEndpointServiceClient,
};
use hsb_mock_endpoints::{EndpointProfile, EndpointRole, run_endpoint};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use ulid::Ulid;

const SYSTEM_COUNT: usize = 10;
const MESSAGE_COUNT_PER_SYSTEM: usize = 300;
const REPORT_PATH: &str = "target/mock-reports/random-non-hl7v3-report.md";
const NATS_DEFAULT_URL: &str = "nats://127.0.0.1:4222";

#[derive(Clone, Copy, Debug, Serialize)]
enum TransportMode {
    Api,
    Grpc,
    Mq,
    Webservice,
}

impl TransportMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Api => "API",
            Self::Grpc => "gRPC",
            Self::Mq => "MQ",
            Self::Webservice => "WebService",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ProtocolSpec {
    protocol_name: &'static str,
    transport: TransportMode,
    content_type: &'static str,
    scenario: &'static str,
}

#[derive(Clone, Debug)]
struct SystemDef {
    endpoint_id: u8,
    service_name: &'static str,
    port: u16,
    role: EndpointRole,
    description: &'static str,
}

#[derive(Clone, Debug, Serialize)]
struct DeliveryTrace {
    sender: &'static str,
    sender_id: u8,
    target: &'static str,
    target_id: u8,
    transport: &'static str,
    protocol: &'static str,
    success: bool,
    latency_ms: u128,
    status: String,
}

#[derive(Clone, Debug, Serialize)]
struct AggregateMetrics {
    attempts: usize,
    success: usize,
    failure: usize,
    success_rate: f64,
    avg_latency_ms: f64,
    min_latency_ms: u128,
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
            role: EndpointRole::Hybrid,
            description: "综合接入外围系统",
        },
        SystemDef {
            endpoint_id: 1,
            service_name: "ext-sys-01-api-clinic",
            port: 9311,
            role: EndpointRole::Hybrid,
            description: "门诊边缘系统",
        },
        SystemDef {
            endpoint_id: 2,
            service_name: "ext-sys-02-ris-core",
            port: 9312,
            role: EndpointRole::Hybrid,
            description: "RIS 核心系统",
        },
        SystemDef {
            endpoint_id: 3,
            service_name: "ext-sys-03-lis-edge",
            port: 9313,
            role: EndpointRole::Hybrid,
            description: "LIS 边缘系统",
        },
        SystemDef {
            endpoint_id: 4,
            service_name: "ext-sys-04-soap-bridge",
            port: 9314,
            role: EndpointRole::Hybrid,
            description: "SOAP 互联桥接",
        },
        SystemDef {
            endpoint_id: 5,
            service_name: "ext-sys-05-emr",
            port: 9315,
            role: EndpointRole::Hybrid,
            description: "EMR 业务系统",
        },
        SystemDef {
            endpoint_id: 6,
            service_name: "ext-sys-06-grpc-adapter",
            port: 9316,
            role: EndpointRole::Hybrid,
            description: "gRPC 适配外围系统",
        },
        SystemDef {
            endpoint_id: 7,
            service_name: "ext-sys-07-grpc-billing",
            port: 9317,
            role: EndpointRole::Hybrid,
            description: "计费外围系统",
        },
        SystemDef {
            endpoint_id: 8,
            service_name: "ext-sys-08-queue-a",
            port: 9318,
            role: EndpointRole::Hybrid,
            description: "队列接入系统 A",
        },
        SystemDef {
            endpoint_id: 9,
            service_name: "ext-sys-09-queue-b",
            port: 9319,
            role: EndpointRole::Hybrid,
            description: "队列接入系统 B",
        },
    ]
}

fn protocol_specs() -> &'static [ProtocolSpec] {
    &[
        ProtocolSpec {
            protocol_name: "FHIR_R5",
            transport: TransportMode::Api,
            content_type: "application/fhir+json",
            scenario: "fhir-rest-random",
        },
        ProtocolSpec {
            protocol_name: "SOAP",
            transport: TransportMode::Webservice,
            content_type: "text/xml; charset=utf-8",
            scenario: "soap-random",
        },
        ProtocolSpec {
            protocol_name: "DICOM",
            transport: TransportMode::Grpc,
            content_type: "application/dicom",
            scenario: "dicom-grpc-random",
        },
        ProtocolSpec {
            protocol_name: "HL7V2",
            transport: TransportMode::Mq,
            content_type: "application/hl7-v2",
            scenario: "hl7v2-mq-random",
        },
        ProtocolSpec {
            protocol_name: "CUSTOM",
            transport: TransportMode::Api,
            content_type: "application/octet-stream",
            scenario: "custom-binary-random",
        },
    ]
}

#[tokio::main]
async fn main() -> Result<()> {
    let systems = system_defs();
    let nats_url = resolve_nats_url();
    let seed = chrono::Utc::now().timestamp_millis() as u64;
    configure_runtime_env(&systems, &nats_url);

    let mut endpoint_handles = Vec::new();
    for system in &systems {
        let profile = EndpointProfile {
            endpoint_id: system.endpoint_id,
            service_name: system.service_name,
            port: system.port,
            role: system.role.clone(),
            description: system.description,
            supported_protocols: &["FHIR_R5", "SOAP", "DICOM", "HL7V2", "CUSTOM"],
            scenario_tags: &[
                "random",
                "non-hl7v3",
                "300-messages",
                "10-system-simulation",
            ],
        };
        endpoint_handles.push(tokio::spawn(async move { run_endpoint(profile).await }));
    }

    let client = Client::builder().timeout(Duration::from_secs(10)).build()?;
    wait_for_readiness(&client, &systems).await?;
    let nats_client = async_nats::connect(nats_url.clone())
        .await
        .with_context(|| format!("failed to connect real NATS broker at {}", nats_url))?;

    let started_at = chrono::Utc::now();
    let started = Instant::now();

    let sender_tasks = systems.iter().map(|sender| {
        let sender = sender.clone();
        let systems = systems.clone();
        let client = client.clone();
        let nats_client = nats_client.clone();
        tokio::spawn(async move { run_sender(sender, systems, client, nats_client, seed).await })
    });

    let traces = join_all(sender_tasks)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    tokio::time::sleep(Duration::from_millis(300)).await;
    let endpoint_stats = fetch_endpoint_stats(&client, &systems).await?;
    let duration = started.elapsed();
    let report = build_report(
        &systems,
        &traces,
        &endpoint_stats,
        &nats_url,
        seed,
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

    let overall = aggregate_metrics(&traces);
    println!("report_path={}", REPORT_PATH);
    println!("duration_ms={}", duration.as_millis());
    println!(
        "logical_messages={}",
        SYSTEM_COUNT * MESSAGE_COUNT_PER_SYSTEM
    );
    println!("delivery_attempts={}", overall.attempts);
    println!("delivery_success={}", overall.success);
    println!("delivery_success_rate={:.2}", overall.success_rate);

    Ok(())
}

async fn run_sender(
    sender: SystemDef,
    systems: Vec<SystemDef>,
    client: Client,
    nats_client: NatsClient,
    seed: u64,
) -> Result<Vec<DeliveryTrace>> {
    let protocols = protocol_specs();
    let mut rng = StdRng::seed_from_u64(seed + sender.endpoint_id as u64 * 97);
    let mut traces = Vec::with_capacity(MESSAGE_COUNT_PER_SYSTEM);

    for message_index in 0..MESSAGE_COUNT_PER_SYSTEM {
        let protocol = protocols[rng.gen_range(0..protocols.len())];
        let target = pick_random_target(&systems, sender.endpoint_id, &mut rng)?;
        let trace = send_message(
            &client,
            &nats_client,
            &sender,
            target,
            protocol,
            message_index,
        )
        .await?;
        traces.push(trace);
    }

    Ok(traces)
}

fn pick_random_target<'a>(
    systems: &'a [SystemDef],
    sender_id: u8,
    rng: &mut StdRng,
) -> Result<&'a SystemDef> {
    let candidates = systems
        .iter()
        .filter(|system| system.endpoint_id != sender_id)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err(anyhow!(
            "no target candidates available for sender {}",
            sender_id
        ));
    }
    Ok(candidates[rng.gen_range(0..candidates.len())])
}

async fn send_message(
    client: &Client,
    nats_client: &NatsClient,
    sender: &SystemDef,
    target: &SystemDef,
    protocol: ProtocolSpec,
    message_index: usize,
) -> Result<DeliveryTrace> {
    match protocol.transport {
        TransportMode::Api => {
            send_http_message(client, sender, target, protocol, message_index).await
        }
        TransportMode::Grpc => send_grpc_message(sender, target, protocol, message_index).await,
        TransportMode::Mq => {
            send_nats_message(nats_client, sender, target, protocol, message_index).await
        }
        TransportMode::Webservice => {
            send_webservice_message(client, sender, target, protocol, message_index).await
        }
    }
}

async fn send_http_message(
    client: &Client,
    sender: &SystemDef,
    target: &SystemDef,
    protocol: ProtocolSpec,
    message_index: usize,
) -> Result<DeliveryTrace> {
    let started = Instant::now();
    let trace_id = Ulid::new().to_string();
    let message_id = Ulid::new().to_string();

    let response = if protocol.protocol_name == "CUSTOM" {
        client
            .post(format!(
                "http://127.0.0.1:{}/api/v1/messages/raw",
                target.port
            ))
            .header("content-type", protocol.content_type)
            .header("x-hsb-protocol", protocol.protocol_name)
            .header("x-hsb-message-type", "CUSTOM.EVENT")
            .header("x-hsb-source", sender.service_name)
            .header("x-hsb-target", target.service_name)
            .header("x-hsb-scenario", protocol.scenario)
            .header("x-trace-id", trace_id)
            .body(build_custom_payload(sender, target, message_index))
            .send()
            .await?
    } else {
        client
            .post(format!("http://127.0.0.1:{}/api/v1/messages", target.port))
            .header("content-type", protocol.content_type)
            .header("x-hsb-protocol", protocol.protocol_name)
            .header("x-trace-id", trace_id.clone())
            .json(&build_api_payload(
                sender,
                target,
                protocol,
                message_index,
                &message_id,
                &trace_id,
            ))
            .send()
            .await?
    };

    let latency_ms = started.elapsed().as_millis();
    let status = response.status().to_string();
    let body = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    Ok(DeliveryTrace {
        sender: sender.service_name,
        sender_id: sender.endpoint_id,
        target: target.service_name,
        target_id: target.endpoint_id,
        transport: protocol.transport.as_str(),
        protocol: protocol.protocol_name,
        success: body
            .get("accepted")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        latency_ms,
        status,
    })
}

async fn send_webservice_message(
    client: &Client,
    sender: &SystemDef,
    target: &SystemDef,
    protocol: ProtocolSpec,
    message_index: usize,
) -> Result<DeliveryTrace> {
    let started = Instant::now();
    let trace_id = Ulid::new().to_string();
    let payload = wrap_soap_envelope(&build_soap_body(sender, target, protocol, message_index));
    let response = client
        .post(format!(
            "http://127.0.0.1:{}/api/v1/messages/raw",
            target.port
        ))
        .header("content-type", protocol.content_type)
        .header("x-hsb-protocol", protocol.protocol_name)
        .header("x-hsb-message-type", "SOAP.EVENT")
        .header("x-hsb-source", sender.service_name)
        .header("x-hsb-target", target.service_name)
        .header("x-hsb-scenario", protocol.scenario)
        .header("x-trace-id", trace_id)
        .body(payload)
        .send()
        .await?;

    let latency_ms = started.elapsed().as_millis();
    let status = response.status().to_string();
    let body = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    Ok(DeliveryTrace {
        sender: sender.service_name,
        sender_id: sender.endpoint_id,
        target: target.service_name,
        target_id: target.endpoint_id,
        transport: protocol.transport.as_str(),
        protocol: protocol.protocol_name,
        success: body
            .get("accepted")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        latency_ms,
        status,
    })
}

async fn send_grpc_message(
    sender: &SystemDef,
    target: &SystemDef,
    protocol: ProtocolSpec,
    message_index: usize,
) -> Result<DeliveryTrace> {
    let started = Instant::now();
    let mut client =
        MockEndpointServiceClient::connect(format!("http://127.0.0.1:{}", target.port))
            .await
            .with_context(|| format!("failed to connect gRPC target {}", target.service_name))?;
    let trace_id = Ulid::new().to_string();
    let mut request = tonic::Request::new(MockMessageRequest {
        message_id: Ulid::new().to_string(),
        protocol: protocol.protocol_name.to_string(),
        message_type: "DICOM.CSTORE".to_string(),
        source: sender.service_name.to_string(),
        target: target.service_name.to_string(),
        scenario: protocol.scenario.to_string(),
        content_type: protocol.content_type.to_string(),
        payload_json: String::new(),
        raw_payload: build_dicom_payload(sender, target, message_index),
        headers: std::collections::HashMap::from([
            ("x-sequence".to_string(), message_index.to_string()),
            (
                "x-protocol-family".to_string(),
                protocol.protocol_name.to_string(),
            ),
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
            Ok(DeliveryTrace {
                sender: sender.service_name,
                sender_id: sender.endpoint_id,
                target: target.service_name,
                target_id: target.endpoint_id,
                transport: protocol.transport.as_str(),
                protocol: protocol.protocol_name,
                success: reply.accepted,
                latency_ms,
                status: reply.status,
            })
        }
        Err(error) => Ok(DeliveryTrace {
            sender: sender.service_name,
            sender_id: sender.endpoint_id,
            target: target.service_name,
            target_id: target.endpoint_id,
            transport: protocol.transport.as_str(),
            protocol: protocol.protocol_name,
            success: false,
            latency_ms,
            status: error.to_string(),
        }),
    }
}

async fn send_nats_message(
    nats_client: &NatsClient,
    sender: &SystemDef,
    target: &SystemDef,
    protocol: ProtocolSpec,
    message_index: usize,
) -> Result<DeliveryTrace> {
    let started = Instant::now();
    let payload = json!({
        "message_id": Ulid::new().to_string(),
        "trace_id": Ulid::new().to_string(),
        "protocol": protocol.protocol_name,
        "message_type": "ADT^A01",
        "source": sender.service_name,
        "target": target.service_name,
        "scenario": protocol.scenario,
        "sequence": message_index,
        "payload_text": build_hl7v2_message(sender, target, message_index),
    })
    .to_string();
    let response = nats_client
        .request(nats_subject(target.endpoint_id), payload.into())
        .await
        .with_context(|| format!("failed to request NATS target {}", target.service_name))?;
    let latency_ms = started.elapsed().as_millis();
    let body = serde_json::from_slice::<Value>(&response.payload)
        .context("failed to decode NATS acknowledgement")?;

    Ok(DeliveryTrace {
        sender: sender.service_name,
        sender_id: sender.endpoint_id,
        target: target.service_name,
        target_id: target.endpoint_id,
        transport: protocol.transport.as_str(),
        protocol: protocol.protocol_name,
        success: body
            .get("accepted")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        latency_ms,
        status: body
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("UNKNOWN")
            .to_string(),
    })
}

fn build_api_payload(
    sender: &SystemDef,
    target: &SystemDef,
    protocol: ProtocolSpec,
    message_index: usize,
    message_id: &str,
    trace_id: &str,
) -> Value {
    match protocol.protocol_name {
        "FHIR_R5" => json!({
            "message_id": message_id,
            "trace_id": trace_id,
            "protocol": protocol.protocol_name,
            "message_type": "Bundle",
            "source": sender.service_name,
            "target": target.service_name,
            "scenario": protocol.scenario,
            "resourceType": "Bundle",
            "type": "message",
            "entry": [{
                "resource": {
                    "resourceType": "Patient",
                    "id": format!("patient-{}-{}", sender.endpoint_id, message_index),
                    "active": true
                }
            }]
        }),
        _ => json!({
            "message_id": message_id,
            "trace_id": trace_id,
            "protocol": protocol.protocol_name,
            "message_type": "GENERIC.EVENT",
            "source": sender.service_name,
            "target": target.service_name,
            "scenario": protocol.scenario,
            "sequence": message_index,
        }),
    }
}

fn build_custom_payload(sender: &SystemDef, target: &SystemDef, message_index: usize) -> Vec<u8> {
    format!(
        "CSTM|{}|{}|{}|{}|{}",
        sender.service_name,
        target.service_name,
        message_index,
        Ulid::new(),
        chrono::Utc::now().timestamp_millis(),
    )
    .into_bytes()
}

fn build_soap_body(
    sender: &SystemDef,
    target: &SystemDef,
    protocol: ProtocolSpec,
    message_index: usize,
) -> String {
    format!(
        "<SubmitMessage><Protocol>{}</Protocol><Sender>{}</Sender><Target>{}</Target><Sequence>{}</Sequence><BusinessId>{}</BusinessId></SubmitMessage>",
        protocol.protocol_name,
        sender.service_name,
        target.service_name,
        message_index,
        Ulid::new(),
    )
}

fn build_dicom_payload(sender: &SystemDef, target: &SystemDef, message_index: usize) -> Vec<u8> {
    format!(
        "DICM|sender={}|target={}|sequence={}|sop_instance_uid={}",
        sender.service_name,
        target.service_name,
        message_index,
        Ulid::new(),
    )
    .into_bytes()
}

fn build_hl7v2_message(sender: &SystemDef, target: &SystemDef, message_index: usize) -> String {
    format!(
        "MSH|^~\\&|{}|HSB|{}|HSB|{}||ADT^A01|{}|P|2.5.1\rPID|1||PAT{}||TEST^PATIENT",
        sender.service_name,
        target.service_name,
        chrono::Utc::now().format("%Y%m%d%H%M%S"),
        Ulid::new(),
        message_index,
    )
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

fn resolve_nats_url() -> String {
    std::env::var("HSB_NATS_URLS").unwrap_or_else(|_| NATS_DEFAULT_URL.to_string())
}

fn configure_runtime_env(systems: &[SystemDef], nats_url: &str) {
    set_env("HSB_MOCK_ACCEPT_RATE", "1.0");
    set_env("HSB_NATS_URLS", nats_url);
    for system in systems {
        set_env(
            &format!("HSB_MOCK_ENDPOINT_{}_NATS_SUBJECT", system.endpoint_id),
            &nats_subject(system.endpoint_id),
        );
    }
}

fn set_env(key: &str, value: &str) {
    unsafe {
        std::env::set_var(key, value);
    }
}

fn nats_subject(endpoint_id: u8) -> String {
    format!("hsb.mock.random.endpoint.{}.inbound", endpoint_id)
}

fn wrap_soap_envelope(payload_xml: &str) -> String {
    format!(
        "<soapenv:Envelope xmlns:soapenv=\"http://schemas.xmlsoap.org/soap/envelope/\"><soapenv:Header/><soapenv:Body>{}</soapenv:Body></soapenv:Envelope>",
        payload_xml
    )
}

fn aggregate_metrics(traces: &[DeliveryTrace]) -> AggregateMetrics {
    let attempts = traces.len();
    let success = traces.iter().filter(|trace| trace.success).count();
    let failure = attempts.saturating_sub(success);
    let latency_sum = traces.iter().map(|trace| trace.latency_ms).sum::<u128>();
    let min_latency_ms = traces
        .iter()
        .map(|trace| trace.latency_ms)
        .min()
        .unwrap_or(0);
    let max_latency_ms = traces
        .iter()
        .map(|trace| trace.latency_ms)
        .max()
        .unwrap_or(0);
    let success_rate = if attempts == 0 {
        0.0
    } else {
        success as f64 * 100.0 / attempts as f64
    };
    let avg_latency_ms = if attempts == 0 {
        0.0
    } else {
        latency_sum as f64 / attempts as f64
    };

    AggregateMetrics {
        attempts,
        success,
        failure,
        success_rate,
        avg_latency_ms,
        min_latency_ms,
        max_latency_ms,
    }
}

fn group_by_protocol(traces: &[DeliveryTrace]) -> BTreeMap<&'static str, AggregateMetrics> {
    let mut buckets = BTreeMap::new();
    for protocol in protocol_specs() {
        let items = traces
            .iter()
            .filter(|trace| trace.protocol == protocol.protocol_name)
            .cloned()
            .collect::<Vec<_>>();
        buckets.insert(protocol.protocol_name, aggregate_metrics(&items));
    }
    buckets
}

fn group_by_sender(traces: &[DeliveryTrace]) -> BTreeMap<&'static str, AggregateMetrics> {
    let mut grouped = BTreeMap::new();
    for system in system_defs() {
        let items = traces
            .iter()
            .filter(|trace| trace.sender == system.service_name)
            .cloned()
            .collect::<Vec<_>>();
        grouped.insert(system.service_name, aggregate_metrics(&items));
    }
    grouped
}

fn build_report(
    systems: &[SystemDef],
    traces: &[DeliveryTrace],
    endpoint_stats: &[EndpointStats],
    nats_url: &str,
    seed: u64,
    started_at: chrono::DateTime<chrono::Utc>,
    duration: Duration,
) -> String {
    let overall = aggregate_metrics(traces);
    let protocol_metrics = group_by_protocol(traces);
    let sender_metrics = group_by_sender(traces);
    let topology_rows = systems
        .iter()
        .map(|system| {
            format!(
                "| {} | {} | {} | {} | {} |",
                system.endpoint_id,
                system.service_name,
                system.port,
                system.role.as_str(),
                system.description,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let protocol_rows = protocol_specs()
        .iter()
        .map(|protocol| {
            let metrics = protocol_metrics
                .get(protocol.protocol_name)
                .cloned()
                .unwrap_or_else(|| aggregate_metrics(&[]));
            format!(
                "| {} | {} | {} | {} | {:.2}% | {:.2} | {} | {} |",
                protocol.protocol_name,
                protocol.transport.as_str(),
                metrics.attempts,
                metrics.success,
                metrics.success_rate,
                metrics.avg_latency_ms,
                metrics.min_latency_ms,
                metrics.max_latency_ms,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let sender_rows = sender_metrics
        .iter()
        .map(|(sender, metrics)| {
            format!(
                "| {} | {} | {} | {:.2}% | {:.2} | {} | {} |",
                sender,
                metrics.attempts,
                metrics.success,
                metrics.success_rate,
                metrics.avg_latency_ms,
                metrics.min_latency_ms,
                metrics.max_latency_ms,
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
    let sample_rows = traces
        .iter()
        .take(20)
        .map(|trace| {
            format!(
                "| {} | {} | {} | {} | {} | {} | {} |",
                trace.sender,
                trace.target,
                trace.protocol,
                trace.transport,
                trace.success,
                trace.latency_ms,
                trace.status,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let suggestions = build_suggestions(traces, endpoint_stats, &protocol_metrics, &overall);

    format!(
        concat!(
            "# 非 HL7 V3 随机并发联调测试报告\n\n",
            "## 测试概览\n\n",
            "- 启动时间: {}\n",
            "- 随机种子: {}\n",
            "- 系统数量: {}\n",
            "- 每个系统发送条数: {}\n",
            "- 总逻辑消息量: {}\n",
            "- 实际投递次数: {}\n",
            "- 成功投递: {}\n",
            "- 失败投递: {}\n",
            "- 成功率: {:.2}%\n",
            "- 总耗时: {} ms\n",
            "- 平均延时: {:.2} ms\n",
            "- 最小延时: {} ms\n",
            "- 最大延时: {} ms\n",
            "- 协议范围: FHIR_R5, SOAP, DICOM, HL7V2, CUSTOM，显式排除 HL7 V3。\n",
            "- MQ 说明: 本次 MQ 使用真实 NATS broker，地址: {}。\n",
            "- 随机策略: 10 个系统全部并发发送，每条消息随机选择目标消费者和协议。\n\n",
            "## 拓扑\n\n",
            "| Endpoint ID | Service | Port | Role | Description |\n",
            "| --- | --- | --- | --- | --- |\n",
            "{}\n\n",
            "## 协议统计\n\n",
            "| Protocol | Transport | Attempts | Success | Success Rate | Avg Latency ms | Min Latency ms | Max Latency ms |\n",
            "| --- | --- | --- | --- | --- | --- | --- | --- |\n",
            "{}\n\n",
            "## 发送方统计\n\n",
            "| Sender | Attempts | Success | Success Rate | Avg Latency ms | Min Latency ms | Max Latency ms |\n",
            "| --- | --- | --- | --- | --- | --- | --- |\n",
            "{}\n\n",
            "## 端点接收统计\n\n",
            "| Endpoint ID | Service | Role | Total | Success | Failure | HTTP | gRPC | NATS |\n",
            "| --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
            "{}\n\n",
            "## 抽样明细\n\n",
            "| Sender | Target | Protocol | Transport | Success | Latency ms | Status |\n",
            "| --- | --- | --- | --- | --- | --- | --- |\n",
            "{}\n\n",
            "## 优化建议\n\n",
            "{}\n"
        ),
        started_at.to_rfc3339(),
        seed,
        systems.len(),
        MESSAGE_COUNT_PER_SYSTEM,
        SYSTEM_COUNT * MESSAGE_COUNT_PER_SYSTEM,
        overall.attempts,
        overall.success,
        overall.failure,
        overall.success_rate,
        duration.as_millis(),
        overall.avg_latency_ms,
        overall.min_latency_ms,
        overall.max_latency_ms,
        nats_url,
        topology_rows,
        protocol_rows,
        sender_rows,
        endpoint_rows,
        sample_rows,
        suggestions,
    )
}

fn build_suggestions(
    traces: &[DeliveryTrace],
    endpoint_stats: &[EndpointStats],
    protocol_metrics: &BTreeMap<&'static str, AggregateMetrics>,
    overall: &AggregateMetrics,
) -> String {
    let mut lines = Vec::new();

    if let Some((protocol, metrics)) = protocol_metrics.iter().max_by(|left, right| {
        left.1
            .avg_latency_ms
            .partial_cmp(&right.1.avg_latency_ms)
            .unwrap()
    }) {
        lines.push(format!(
            "- {} 的平均延时为 {:.2} ms，是本轮最慢协议，建议优先检查该协议链路的连接复用、序列化成本以及目标端处理队列。",
            protocol,
            metrics.avg_latency_ms,
        ));
    }

    if overall.max_latency_ms > overall.avg_latency_ms as u128 * 3 {
        lines.push(format!(
            "- 最大延时达到 {} ms，明显高于平均值 {:.2} ms，建议在发送端增加直方图监控，并拆分 DNS、连接建立、应用处理三个阶段的耗时。",
            overall.max_latency_ms,
            overall.avg_latency_ms,
        ));
    } else {
        lines.push(format!(
            "- 本轮最大延时 {} ms 与平均值 {:.2} ms 的偏差仍可观察，建议继续用更长时间窗口压测，关注尾延时在更高并发下是否抬升。",
            overall.max_latency_ms,
            overall.avg_latency_ms,
        ));
    }

    if let Some(max_endpoint) = endpoint_stats.iter().max_by_key(|stats| stats.total) {
        let avg_endpoint_load = endpoint_stats.iter().map(|stats| stats.total).sum::<u64>() as f64
            / endpoint_stats.len() as f64;
        lines.push(format!(
            "- {} 接收 {} 条消息，端点平均负载为 {:.2}，建议持续监控热点端点并在生产调度策略中加入负载均衡权重。",
            max_endpoint.service_name,
            max_endpoint.total,
            avg_endpoint_load,
        ));
    }

    let custom_metrics = protocol_metrics
        .get("CUSTOM")
        .cloned()
        .unwrap_or_else(|| aggregate_metrics(&[]));
    if custom_metrics.attempts > 0 {
        lines.push(format!(
            "- 自定义协议已完成 {} 次投递，建议为 CUSTOM 链路补齐固定帧格式、版本号和校验位，并为异常报文增加独立拒收码，避免后续协议演进时出现兼容性漂移。",
            custom_metrics.attempts,
        ));
    }

    if traces.iter().all(|trace| trace.success) {
        lines.push("- 本轮成功率为 100%，当前更适合追加故障注入测试，例如降低部分端点 accept rate、增加 NATS 背压和超时阈值验证，以确认重试与熔断策略是否足够稳健。".to_string());
    } else {
        lines.push(format!(
            "- 本轮存在 {} 次失败投递，建议结合失败状态码按协议拆分错误面板，并对重试可恢复错误与协议格式错误分别处理。",
            overall.failure,
        ));
    }

    lines.join("\n")
}
