# HSB Mock Endpoints

这组测试服务用于模拟 5 个独立的上下游系统 endpoint，默认监听如下端口：

- endpoint-0: 9200
- endpoint-1: 9201
- endpoint-2: 9202
- endpoint-3: 9203
- endpoint-4: 9204

每个服务都具备以下能力：

- HTTP 接口：同端口提供 `/health`、`/api/v1/capabilities`、`/api/v1/stats`、`/api/v1/messages`
- gRPC 接口：同端口提供 `mockendpoint.MockEndpointService/HandleMessage`
- 随机响应：70% 成功，30% 失败
- 随机失败原因：数据库超时、系统繁忙、消息解析错误、模式校验失败、重复消息、下游不可用、消息队列背压、鉴权过期、路由失败、协议桥接失败、限流、主数据缺失
- 可选生产者模式：通过环境变量定时向 HTTP、gRPC 或 NATS 目标发送测试消息
- 可选 NATS 消费模式：通过环境变量订阅 subject，接收消息并回 Ack

## 默认角色分工

- endpoint-0：生产者型，适合 REST 入站和单/多生产者场景
- endpoint-1：消费者型，适合严格校验和单/多消费者场景
- endpoint-2：混合型，适合生产者+消费者和编排回调场景
- endpoint-3：消费者型，偏 gRPC/NATS，适合异步下游波动场景
- endpoint-4：混合型，适合协议桥接和同/异协议联调场景

## 启动方式

```bash
cargo run -p hsb-mock-endpoints --bin hsb-endpoint-0
cargo run -p hsb-mock-endpoints --bin hsb-endpoint-1
cargo run -p hsb-mock-endpoints --bin hsb-endpoint-2
cargo run -p hsb-mock-endpoints --bin hsb-endpoint-3
cargo run -p hsb-mock-endpoints --bin hsb-endpoint-4
```

## HTTP 示例

```bash
curl -X POST http://127.0.0.1:9200/api/v1/messages \
  -H 'Content-Type: application/json' \
  -H 'X-HSB-Protocol: REST' \
  -H 'X-HSB-Message-Type: ADT^A01' \
  -d '{"message_id":"demo-1","source":"his","target":"lis","scenario":"single-producer","payload":{"patient_id":"P001"}}'
```

## 可选环境变量

- `HSB_MOCK_ENDPOINT_<id>_PORT`：覆盖默认端口
- `HSB_MOCK_ENDPOINT_<id>_PRODUCER_TARGET`：开启主动生产消息，值为 HTTP URL、gRPC 地址或 NATS subject
- `HSB_MOCK_ENDPOINT_<id>_PRODUCER_PROTOCOL`：`http`、`grpc`、`nats`
- `HSB_MOCK_ENDPOINT_<id>_PRODUCER_INTERVAL_MS`：主动推送间隔，默认 5000
- `HSB_MOCK_ENDPOINT_<id>_NATS_URL`：NATS 地址，默认取 `HSB_NATS_URLS`，再退回 `nats://127.0.0.1:4222`
- `HSB_MOCK_ENDPOINT_<id>_NATS_SUBJECT`：开启 NATS 消费模式
- `HSB_MOCK_ENDPOINT_<id>_NATS_ACK_SUBJECT`：没有 reply subject 时，用于发送 Ack 的备用 subject

## 说明

目前先提供通用测试接口，便于你后续补充具体业务 API。收到后我可以在这 5 个独立程序上继续扩展你指定的接口语义、协议格式和场景脚本。