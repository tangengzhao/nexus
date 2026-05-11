#[derive(Debug, Clone)]
pub enum EndpointRole {
    Producer,
    Consumer,
    Hybrid,
}

impl EndpointRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Producer => "producer",
            Self::Consumer => "consumer",
            Self::Hybrid => "hybrid",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EndpointProfile {
    pub endpoint_id: u8,
    pub service_name: &'static str,
    pub port: u16,
    pub role: EndpointRole,
    pub description: &'static str,
    pub supported_protocols: &'static [&'static str],
    pub scenario_tags: &'static [&'static str],
}

pub fn profile_by_id(endpoint_id: u8) -> Option<EndpointProfile> {
    match endpoint_id {
        0 => Some(EndpointProfile {
            endpoint_id,
            service_name: "endpoint-0-ingress-producer",
            port: 9200,
            role: EndpointRole::Producer,
            description: "入站生产者型服务，偏 REST/JSON，适合单生产者或多生产者并发发送测试。",
            supported_protocols: &["REST", "JSON", "HL7v2", "FHIR"],
            scenario_tags: &["single-producer", "multi-producer", "rest-ingress"],
        }),
        1 => Some(EndpointProfile {
            endpoint_id,
            service_name: "endpoint-1-validation-consumer",
            port: 9201,
            role: EndpointRole::Consumer,
            description: "消费校验型服务，偏严格校验，适合单消费者或多消费者分发测试。",
            supported_protocols: &["REST", "JSON", "SOAP", "FHIR"],
            scenario_tags: &["single-consumer", "multi-consumer", "strict-validation"],
        }),
        2 => Some(EndpointProfile {
            endpoint_id,
            service_name: "endpoint-2-hybrid-orchestrator",
            port: 9202,
            role: EndpointRole::Hybrid,
            description: "双向混合型服务，同时支持接收和主动推送，适合编排与回调场景。",
            supported_protocols: &["REST", "gRPC", "JSON", "HL7v2", "NATS"],
            scenario_tags: &["hybrid", "producer-consumer", "orchestration"],
        }),
        3 => Some(EndpointProfile {
            endpoint_id,
            service_name: "endpoint-3-grpc-consumer",
            port: 9203,
            role: EndpointRole::Consumer,
            description: "偏 gRPC 与消息接入的消费者型服务，适合异步和下游不稳定场景。",
            supported_protocols: &["gRPC", "JSON", "FHIR", "NATS"],
            scenario_tags: &["grpc-ingress", "async-consumer", "transient-failure"],
        }),
        4 => Some(EndpointProfile {
            endpoint_id,
            service_name: "endpoint-4-protocol-bridge",
            port: 9204,
            role: EndpointRole::Hybrid,
            description: "协议桥接型服务，适合同协议和异协议转换前后的联调压测。",
            supported_protocols: &["REST", "gRPC", "JSON", "XML", "HL7v2", "FHIR", "SOAP"],
            scenario_tags: &[
                "protocol-bridge",
                "same-protocol",
                "cross-protocol",
                "hybrid",
            ],
        }),
        _ => None,
    }
}
