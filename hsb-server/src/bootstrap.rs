//! 服务器引导

use std::sync::Arc;
use tracing::{info, warn};

use hsb_adapter::dicom::DicomAdapter;
use hsb_adapter::fhir::FhirR5Adapter;
use hsb_adapter::hl7::Hl7V2Adapter;
use hsb_adapter::hl7v3::Hl7V3Adapter;
use hsb_adapter::soap::SoapAdapter;
use hsb_core::{ConnectableTransport, TransportRegistry};

use crate::config::ServerConfig;

/// 注册所有协议适配器
#[allow(dead_code)]
pub fn register_adapters(_config: &ServerConfig) -> hsb_core::engine::AdapterRegistry {
    let mut registry = hsb_core::engine::AdapterRegistry::new();

    // HL7 v2.x 适配器
    let hl7_adapter = Arc::new(Hl7V2Adapter::new());
    registry.register(hl7_adapter);
    info!("Registered HL7 v2.x adapter");

    let hl7v3_adapter = Arc::new(Hl7V3Adapter::new());
    registry.register(hl7v3_adapter);
    info!("Registered HL7 v3 adapter");

    // FHIR R5 适配器
    let fhir_adapter = Arc::new(FhirR5Adapter::new());
    registry.register(fhir_adapter);
    info!("Registered FHIR R5 adapter");

    // DICOM 适配器
    let dicom_adapter = Arc::new(DicomAdapter::new());
    registry.register(dicom_adapter);
    info!("Registered DICOM adapter");

    // SOAP 适配器
    let soap_adapter = Arc::new(SoapAdapter::new());
    registry.register(soap_adapter);
    info!("Registered SOAP adapter");

    info!(
        "Registered {} protocol adapters: {:?}",
        registry.supported_protocols().len(),
        registry.supported_protocols()
    );

    registry
}

/// 注册所有传输层
pub async fn register_transports(
    config: &ServerConfig,
) -> (
    TransportRegistry,
    Option<Arc<hsb_transport::kafka::KafkaTransport>>,
) {
    let mut registry = TransportRegistry::new();
    let mut kafka_handle = None;

    // HTTP 传输
    if config.http.enabled {
        let http_config = hsb_transport::http::HttpTransportConfig {
            name: "http".to_string(),
            base_url: Some(format!(
                "http://{}:{}",
                config.http.listen_address, config.http.port
            )),
            timeout_secs: config.http.request_timeout_secs,
            ..Default::default()
        };
        if let Ok(http_transport) = hsb_transport::http::HttpTransport::new(http_config) {
            registry.register("http", Arc::new(http_transport));
            info!("Registered HTTP transport");
        }
    }

    // TCP/MLLP 传输
    if config.tcp.enabled {
        let tcp_config = hsb_transport::tcp::TcpTransportConfig {
            name: "tcp".to_string(),
            host: config.tcp.listen_address.clone(),
            port: config.tcp.port,
            connect_timeout_secs: config.tcp.connection_timeout_secs,
            timeout_secs: 30,
            ..Default::default()
        };
        let tcp_transport = Arc::new(hsb_transport::tcp::TcpMllpTransport::new(tcp_config));
        registry.register("tcp", tcp_transport.clone());
        registry.register("mllp", tcp_transport);
        info!("Registered TCP/MLLP transport");
    }

    // gRPC 传输
    if config.grpc.enabled {
        let grpc_config = hsb_transport::grpc::GrpcTransportConfig {
            endpoint: format!("http://{}:{}", config.grpc.listen_address, config.grpc.port),
            connect_timeout_secs: 10,
            request_timeout_secs: 30,
            ..Default::default()
        };
        let grpc_transport = Arc::new(hsb_transport::grpc::GrpcTransport::new(grpc_config));
        registry.register("grpc", grpc_transport);
        info!("Registered gRPC transport");
    }

    // MQ 传输
    if config.mq.enabled {
        let mq_config = hsb_transport::mq::MqTransportConfig {
            name: "rabbitmq".to_string(),
            host: config.mq.host.clone(),
            port: config.mq.port,
            username: config.mq.username.clone(),
            password: config.mq.password.clone(),
            vhost: config.mq.vhost.clone(),
            prefetch_count: config.mq.prefetch_count,
            ..Default::default()
        };
        let mq_transport = Arc::new(hsb_transport::mq::MqTransport::new(mq_config));
        if let Err(e) = mq_transport.connect().await {
            warn!("Failed to connect to external MQ: {}", e);
        }
        registry.register("mq", mq_transport.clone());
        registry.register("amqp", mq_transport.clone());
        registry.register("rabbitmq", mq_transport);
        info!("Registered MQ transport");
    }

    // Kafka 传输
    if config.kafka.enabled {
        let kafka_config = hsb_transport::kafka::KafkaTransportConfig {
            name: "kafka".to_string(),
            bootstrap_servers: config.kafka.bootstrap_servers.clone(),
            client_id: config.kafka.client_id.clone(),
            default_topic: config.kafka.default_topic.clone(),
            security_protocol: config.kafka.security_protocol.clone(),
            sasl_username: config.kafka.sasl_username.clone(),
            sasl_password: config.kafka.sasl_password.clone(),
            sasl_mechanism: config.kafka.sasl_mechanism.clone(),
            socket_timeout_secs: config.kafka.socket_timeout_secs,
            message_timeout_secs: config.kafka.message_timeout_secs,
            consumer: hsb_transport::kafka::KafkaConsumerConfig {
                group_id: config.kafka.consumer.group_id.clone(),
                topics: config.kafka.consumer.topics.clone(),
                start_from_earliest: config.kafka.consumer.start_from_earliest,
                session_timeout_secs: config.kafka.consumer.session_timeout_secs,
            },
        };

        let kafka_transport = Arc::new(hsb_transport::kafka::KafkaTransport::new(kafka_config));
        if let Err(e) = kafka_transport.connect().await {
            warn!("Failed to connect to external Kafka: {}", e);
        } else if !config.kafka.consumer.topics.is_empty() {
            let topics = config.kafka.consumer.topics.clone();
            if let Err(e) = kafka_transport
                .start_consumer(&topics, |message| async move {
                    info!(
                        "Kafka message received: topic={}, partition={}, offset={}, bytes={}",
                        message.topic,
                        message.partition,
                        message.offset,
                        message.payload.len(),
                    );
                    Ok(())
                })
                .await
            {
                warn!("Failed to start Kafka consumer loop: {}", e);
            } else {
                info!("Kafka consumer started for topics: {:?}", topics);
            }
        }
        kafka_handle = Some(kafka_transport.clone());
        registry.register("kafka", kafka_transport);
        info!("Registered Kafka transport");
    }

    // NATS 传输
    if config.nats.enabled {
        let nats_config = hsb_transport::nats::NatsTransportConfig {
            name: "nats".to_string(),
            urls: config.nats.urls.clone(),
            username: config.nats.username.clone(),
            password: config.nats.password.clone(),
            token: config.nats.token.clone(),
            subject_prefix: config.nats.subject_prefix.clone(),
            jetstream: hsb_transport::nats::JetStreamConfig {
                enabled: config.nats.jetstream_enabled,
                default_stream: config.nats.jetstream_stream.clone(),
                stream_subjects: config.nats.jetstream_subjects.clone(),
                max_age_secs: config.nats.max_age_secs,
                ..Default::default()
            },
            ..Default::default()
        };
        let nats_transport = Arc::new(hsb_transport::nats::NatsTransport::new(nats_config));
        if let Err(e) = nats_transport.connect().await {
            warn!("Failed to connect to external NATS: {}", e);
        }
        registry.register("nats", nats_transport.clone());
        registry.register("jetstream", nats_transport);
        info!("Registered NATS/JetStream transport");
    }

    info!("Registered {} transports", registry.list().len());

    (registry, kafka_handle)
}

/// 初始化 SSO 客户端
#[allow(dead_code)]
pub async fn init_sso_client(config: &ServerConfig) -> Option<Arc<hsb_transport::grpc::SsoClient>> {
    if !config.sso.enabled {
        return None;
    }

    let sso_config = hsb_transport::grpc::SsoClientConfig {
        endpoint: config.sso.endpoint.clone(),
        client_id: config.sso.client_id.clone(),
        client_secret: config.sso.client_secret.clone(),
        token_cache_secs: config.sso.token_cache_secs,
        ..Default::default()
    };

    let sso_client = Arc::new(hsb_transport::grpc::SsoClient::new(sso_config));

    if let Err(e) = sso_client.connect().await {
        tracing::warn!("Failed to connect to SSO service: {}", e);
    } else {
        info!("Connected to SSO service at {}", config.sso.endpoint);
    }

    Some(sso_client)
}
