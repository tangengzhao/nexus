# Nexus HSB 外围系统对接开发指南

> 版本：1.0 · 适用系统：Nexus HSB v1.x  
> 面向读者：HIS、LIS、PACS、EMR、药房、结算等业务系统的开发工程师

---

## 目录

1. [系统概念](#1-系统概念)
2. [接入架构](#2-接入架构)
3. [支持的协议与传输通道](#3-支持的协议与传输通道)
4. [接入方式与 API 说明](#4-接入方式与-api-说明)
5. [消息格式与必要 Header](#5-消息格式与必要-header)
6. [接入步骤](#6-接入步骤)
7. [Java 示例代码](#7-java-示例代码)
8. [Rust 示例代码](#8-rust-示例代码)
9. [Python 示例代码](#9-python-示例代码)
10. [Golang 示例代码](#10-golang-示例代码)
11. [错误码与排障](#11-错误码与排障)
12. [最佳实践](#12-最佳实践)

---

## 1. 系统概念

### 1.1 什么是 Nexus HSB

**Nexus HSB（Hospital Service Bus，医院服务总线）** 是一个面向医疗机构集成场景的企业级消息中间件平台。它的核心职责是：

- 充当各医疗业务系统之间的**消息枢纽**，解耦系统间直连关系
- 统一处理 HL7、FHIR、DICOM、SOAP、HTTP/JSON、Webhook、OpenAI 兼容接口等**异构协议**的接入与转换
- 提供可靠的消息**路由、投递、重试、死信**机制
- 记录全链路**审计轨迹**，满足医疗合规要求

```
  HIS ──┐
  LIS ──┤                                     ┌── HIS
 PACS ──┤──→  Nexus HSB（协议适配 + 路由）──→ ├── LIS
  EMR ──┤                                     └── PACS
  其他──┘
```

### 1.2 核心概念

| 概念 | 说明 |
|------|------|
| **系统（System）** | 一个参与集成的业务系统，如 HIS、LIS、PACS。每个系统有唯一 `system_id`，是消息路由的身份标识 |
| **机构（Organization）** | 系统所属的医院或科室，用于多院区、多机构场景 |
| **端点（Endpoint）** | 某个系统的具体连接配置（地址、认证、协议），一个系统可有多个端点（如 HTTP、Webhook、MQ、OpenAI、数据库配置型端点）|
| **路由（Route）** | 定义"来自哪个系统的什么类型消息，投递到哪些目标端点"，是消息流转的核心规则 |
| **Topic** | 消息的业务分类标签，格式为 `<domain>.<service>.<action>.<version>`，如 `medical.order.create.v1` |
| **消息（Message）** | HSB 内部统一消息对象，携带原始报文、解析后的 JSON payload、路由元信息 |
| **死信队列（DLQ）** | 多次重试失败的消息进入死信队列，可手动重放 |
| **自定义协议** | 当标准协议不满足时，可通过管理界面定义私有报文结构 |
| **Webhook 端点** | 面向外围系统的事件回调端点，当前按 HTTP POST 投递，可记录事件类型和签名配置 |
| **OpenAI 端点** | OpenAI 兼容消费者端点，用于把消息投递到 Chat Completions、Responses、Embeddings 等接口 |
| **数据库端点** | 数据库直连配置型端点，目前支持类型、数据库名、Schema、JDBC URL 等配置建模；运行态 SQL 投递执行层尚未完成 |

### 1.3 消息流转全链路

```
外部系统发送报文
       ↓
  HTTP / TCP / MQ 入站接口
       ↓
  协议自动识别（HL7/FHIR/JSON/SOAP…）
       ↓
  协议适配器解析 → 生成统一 Message 对象
       ↓
  处理管道（日志、指标、校验）
       ↓
  路由引擎匹配规则
       ↓
    投递到目标端点（HTTP / Webhook / TCP / MQ / gRPC / OpenAI）
       ↓
  结果记录（审计 + 持久化）
```

---

## 2. 接入架构

### 2.1 服务地址

| 服务 | 默认端口 | 说明 |
|------|----------|------|
| HTTP 入站 & UI | **8080** | 业务消息接收 + 管理台 |
| Admin API | **8081** | 配置管理 REST API（可选，主端口同样提供） |
| TCP / MLLP | **2575** | HL7 v2.x 标准 MLLP 端口 |
| gRPC | **10051** | gRPC 入站 |

> 所有 HTTP 接口均挂载在主 8080 端口，`/api/v1/` 为管理 API，`/api/messages/inbound` 为业务消息入站接口。

### 2.2 两种接入角色

**主动发送方（Producer）**：业务系统产生事件后，主动 POST 到 HSB 入站接口，由 HSB 完成路由和投递。

**被动接收方（Consumer）**：HSB 根据路由配置，将消息推送（HTTP 回调 / Webhook / MQ / TCP / OpenAI 兼容接口）到目标系统，目标系统需要提供接收端点。

大部分系统同时扮演两种角色。

---

## 3. 支持的协议与传输通道

### 3.1 支持的消息协议

| 协议标识 | 说明 | 典型场景 |
|---------|------|---------|
| `HTTP` | HTTP/JSON，通用 REST 消息 | 现代系统接入、自定义业务消息 |
| `WEBHOOK` | Webhook 回调，按 HTTP POST 投递，可记录事件类型与签名配置 | 外围系统事件订阅、异步回调通知 |
| `HL7V2` | HL7 v2.x，MLLP 封装 | HIS/LIS 传统接口（ADT、ORM、ORU 等） |
| `HL7V3` | HL7 v3 XML（CDA） | 区域卫生信息平台、CDA 文档交换 |
| `FHIR_R5` | HL7 FHIR R5 | 互联互通、移动端、标准化接口 |
| `DICOM` | DICOM 影像通信 | PACS/RIS 影像传输 |
| `SOAP` | SOAP 1.1/1.2 WebService | 老系统 WebService 接口 |
| `OPENAI` | OpenAI 兼容消费者端点，运行时按 HTTP 投递 | AI 辅助质控、报告摘要、结构化抽取、Embedding 生成 |
| `DATABASE` | 数据库直连配置型端点，目前支持配置建模，运行投递执行层尚未完成 | 记录外部库连接信息、后续直连查询/写入扩展 |
| `CUSTOM` | 用户自定义报文结构 | 私有协议接入 |

### 3.2 支持的传输通道

| 通道 | 描述 |
|------|------|
| HTTP/HTTPS | 标准 REST，推荐首选；Webhook 和 OpenAI 当前也复用 HTTP transport |
| TCP/MLLP | HL7 v2.x 标准传输，端口 2575 |
| RabbitMQ (AMQP) | 异步消息队列，适合高并发、解耦场景 |
| Kafka | 高吞吐流式消息 |
| NATS | 轻量级高性能消息中间件 |
| gRPC | 高性能 RPC，适合内部服务间通信 |

### 3.3 协议自动识别规则

发送 HTTP 请求时，HSB 按以下顺序自动识别协议，**无需手动指定**：

1. 若请求头包含 `X-HSB-Protocol`，优先以该值为准
2. `Content-Type: application/fhir+json` → FHIR R5
3. 请求体以 `MSH|` 开头 → HL7 v2.x
4. 请求体为包含 `urn:hl7-org:v3` 的 XML → HL7 v3
5. 请求体包含 SOAP Envelope → SOAP
6. `Content-Type: application/dicom` → DICOM
7. 其他情况 → HTTP/JSON（默认）

---

## 4. 接入方式与 API 说明

### 4.1 推荐接入方式：HTTP 入站

**最简接入方式**，适用于所有能发 HTTP 请求的系统。

```
POST http://<hsb-host>:8080/api/messages/inbound
```

**必填请求头：**

| Header | 说明 | 示例 |
|--------|------|------|
| `X-HSB-Source-System` | **必填**，发送方系统 ID（在管理台注册） | `his-inpatient` |
| `Content-Type` | 消息格式，影响协议自动识别 | `application/json` |

**可选请求头：**

| Header | 说明 | 示例 |
|--------|------|------|
| `X-HSB-Target-System` | 指定目标系统 ID（不指定则由路由规则决定） | `lis-main` |
| `X-HSB-Message-Type` | 消息类型，辅助路由匹配 | `ADT_A01` / `ORDER_CREATE` |
| `X-HSB-Trace-Id` | 调用方自定义追踪 ID，全链路透传 | `trace-20240512-001` |
| `X-HSB-Correlation-Id` | 关联 ID，用于请求-响应配对 | `req-uuid-xxx` |
| `X-HSB-Priority` | 消息优先级 | `high` / `normal` / `low` |
| `X-HSB-Protocol` | 强制指定协议（覆盖自动识别） | `HL7V2` |

**成功响应（HTTP 202）：**

```json
{
  "message_id": "01HXXXXXXXXXXXX",
  "trace_id": "trace-20240512-001",
  "protocol": "HTTP",
  "matched_routes": ["route-id-001"],
  "deliveries": [
    {
      "route_id": "route-id-001",
      "target": "lis-main",
      "success": true,
      "duration_ms": 45,
      "error": null
    }
  ]
}
```

### 4.2 管理配置 API

配置类操作均通过 Admin API 完成，前缀为 `/api/v1/`。

**主要资源端点：**

| 资源 | 方法 | 路径 | 说明 |
|------|------|------|------|
| 机构 | GET/POST | `/api/v1/organizations` | 创建/查询机构 |
| 系统 | GET/POST | `/api/v1/systems` | 注册/查询业务系统 |
| 端点 | GET/POST | `/api/v1/endpoints` | 创建/查询端点 |
| 端点状态 | PUT | `/api/v1/endpoints/{id}/status` | 启停端点 |
| 健康检查 | GET | `/api/v1/endpoints/{id}/health` | 检查端点连通性 |
| 路由 | GET/POST | `/api/v1/routes` | 创建/查询路由 |
| 路由启用 | POST | `/api/v1/routes/{id}/enable` | 启用路由 |
| 消息查询 | GET | `/api/v1/messages` | 查询历史消息 |
| 消息重处理 | POST | `/api/v1/messages/{id}/reprocess` | 重新处理消息 |
| 死信队列 | GET | `/api/v1/dlq` | 查询死信消息 |
| 死信重放 | POST | `/api/v1/dlq/{id}/reprocess` | 重放死信消息 |
| 审计日志 | GET | `/api/v1/audit` | 查询审计记录 |
| 消息追踪 | GET | `/api/v1/audit/trace/{message_id}` | 查看消息完整链路 |
| 健康 | GET | `/health` | 服务健康检查 |
| 就绪 | GET | `/api/v1/ready` | 服务就绪检查 |

### 4.3 注册系统与端点（快速流程）

```
1. POST /api/v1/organizations     创建机构（已有则跳过）
2. POST /api/v1/systems           注册业务系统，获得 system_id
3. POST /api/v1/endpoints         为系统创建目标端点（接收消息的地址）
4. POST /api/v1/routes            配置路由规则（来源 → 目标）
5. 启动业务系统，开始发送消息到 /api/messages/inbound
```

---

## 5. 消息格式与必要 Header

### 5.1 HTTP/JSON 通用消息

```http
POST /api/messages/inbound HTTP/1.1
Host: hsb-server:8080
Content-Type: application/json
X-HSB-Source-System: his-inpatient
X-HSB-Message-Type: PATIENT_ADMIT
X-HSB-Trace-Id: your-trace-id-001

{
  "patient_id": "P100001",
  "patient_name": "张三",
  "admit_time": "2026-05-12T08:00:00+08:00",
  "department": "内科",
  "bed_no": "12A"
}
```

### 5.2 HL7 v2.x 消息（HTTP 传输）

```http
POST /api/messages/inbound HTTP/1.1
Host: hsb-server:8080
Content-Type: text/plain; charset=utf-8
X-HSB-Source-System: his-inpatient
X-HSB-Protocol: HL7V2

MSH|^~\&|HIS|HOSPITAL|HSB|NEXUS|20260512080000||ADT^A01|MSG001|P|2.5
EVN|A01|20260512080000
PID|1||P100001^^^HOSPITAL||张三||19800101|M
PV1|1|I|内科^12A^床||||||||||||||||||V001
```

### 5.3 HL7 v2.x 消息（TCP/MLLP 传输）

MLLP 使用特殊控制字符封装报文：`0x0B` + HL7报文体 + `0x1C` + `0x0D`。

```
[0x0B]MSH|^~\&|HIS|...[0x1C][0x0D]
```

---

## 6. 接入步骤

### 第一步：注册机构（若尚未创建）

```bash
curl -X POST http://hsb-server:8081/api/v1/organizations \
  -H "Content-Type: application/json" \
  -d '{
    "name": "某某医院",
    "organization_type": "HOSPITAL",
    "description": "某某医院集成平台"
  }'
```

### 第二步：注册业务系统

```bash
curl -X POST http://hsb-server:8081/api/v1/systems \
  -H "Content-Type: application/json" \
  -d '{
    "organization_id": "<上一步返回的 id>",
    "name": "住院管理系统",
    "system_type": "HIS",
    "description": "住院 HIS，负责床位、入出转业务"
  }'
# 记录返回的 system id，即 source_system 标识
```

### 第三步：为目标系统创建接收端点

```bash
curl -X POST http://hsb-server:8081/api/v1/endpoints \
  -H "Content-Type: application/json" \
  -d '{
    "organization_id": "<机构 id>",
    "system_id": "<目标系统 id>",
    "name": "LIS HTTP 接收端点",
    "system_type": "LIS",
    "protocol": "HTTP",
    "connection": {
      "address": "http://lis-server:9200",
      "path": "/api/receive",
      "timeout_secs": 30
    }
  }'
```

### 第四步：创建路由规则

```bash
curl -X POST http://hsb-server:8081/api/v1/routes \
  -H "Content-Type: application/json" \
  -d '{
    "name": "HIS→LIS 检验申请路由",
    "source_system_id": "<HIS 系统 id>",
    "enabled": true,
    "source_match": {
      "message_type": "ORDER_CREATE"
    },
    "targets": [
      {
        "endpoint_id": "<LIS 端点 id>",
        "delivery_mode": "AT_LEAST_ONCE"
      }
    ]
  }'
```

### 第五步：发送消息

在业务代码中，向 HSB 发送消息即可，路由规则会自动将消息投递到 LIS。

---

## 7. Java 示例代码

以下示例使用 Java 11+（`java.net.http.HttpClient`），无需额外依赖。

### 7.1 通用 HTTP 消息发送工具类

```java
package com.example.hsb;

import com.fasterxml.jackson.databind.ObjectMapper;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.Map;
import java.util.UUID;

public class HsbClient {

    private final HttpClient httpClient;
    private final String hsbBaseUrl;
    private final String sourceSystemId;
    private final ObjectMapper objectMapper;

    public HsbClient(String hsbBaseUrl, String sourceSystemId) {
        this.hsbBaseUrl = hsbBaseUrl.replaceAll("/$", "");
        this.sourceSystemId = sourceSystemId;
        this.objectMapper = new ObjectMapper();
        this.httpClient = HttpClient.newBuilder()
                .connectTimeout(Duration.ofSeconds(10))
                .build();
    }

    /**
     * 发送 JSON 消息到 HSB
     *
     * @param messageType 消息类型，如 "ORDER_CREATE"
     * @param payload     业务报文对象（将被序列化为 JSON）
     * @return HSB 响应
     */
    public HsbResponse sendJson(String messageType, Object payload) throws Exception {
        return sendJson(messageType, payload, null);
    }

    public HsbResponse sendJson(String messageType, Object payload, String targetSystem) throws Exception {
        String traceId = "hsb-" + UUID.randomUUID();
        String body = objectMapper.writeValueAsString(payload);

        HttpRequest.Builder requestBuilder = HttpRequest.newBuilder()
                .uri(URI.create(hsbBaseUrl + "/api/messages/inbound"))
                .timeout(Duration.ofSeconds(30))
                .header("Content-Type", "application/json")
                .header("X-HSB-Source-System", sourceSystemId)
                .header("X-HSB-Message-Type", messageType)
                .header("X-HSB-Trace-Id", traceId)
                .POST(HttpRequest.BodyPublishers.ofString(body));

        if (targetSystem != null && !targetSystem.isEmpty()) {
            requestBuilder.header("X-HSB-Target-System", targetSystem);
        }

        HttpResponse<String> response = httpClient.send(
                requestBuilder.build(),
                HttpResponse.BodyHandlers.ofString()
        );

        if (response.statusCode() == 202) {
            return objectMapper.readValue(response.body(), HsbResponse.class);
        } else {
            throw new HsbException(response.statusCode(), response.body());
        }
    }

    /**
     * 发送 HL7 v2.x 消息（HTTP 传输方式，不使用 MLLP）
     */
    public HsbResponse sendHl7v2(String hl7Message) throws Exception {
        String traceId = "hsb-" + UUID.randomUUID();

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(hsbBaseUrl + "/api/messages/inbound"))
                .timeout(Duration.ofSeconds(30))
                .header("Content-Type", "text/plain; charset=utf-8")
                .header("X-HSB-Source-System", sourceSystemId)
                .header("X-HSB-Protocol", "HL7V2")
                .header("X-HSB-Trace-Id", traceId)
                .POST(HttpRequest.BodyPublishers.ofString(hl7Message))
                .build();

        HttpResponse<String> response = httpClient.send(
                request, HttpResponse.BodyHandlers.ofString()
        );

        if (response.statusCode() == 202) {
            return objectMapper.readValue(response.body(), HsbResponse.class);
        } else {
            throw new HsbException(response.statusCode(), response.body());
        }
    }

    // ---- 内部类 ----

    public static class HsbResponse {
        public String messageId;
        public String traceId;
        public String protocol;
        public java.util.List<String> matchedRoutes;
        // Jackson 会将 snake_case 自动映射，或使用 @JsonProperty
    }

    public static class HsbException extends RuntimeException {
        public final int statusCode;
        public final String body;

        public HsbException(int statusCode, String body) {
            super("HSB error " + statusCode + ": " + body);
            this.statusCode = statusCode;
            this.body = body;
        }
    }
}
```

### 7.2 业务使用示例

```java
package com.example.his;

import com.example.hsb.HsbClient;
import java.util.Map;

public class PatientAdmitService {

    // 推荐注入为 Spring Bean（单例，线程安全）
    private final HsbClient hsbClient = new HsbClient(
            "http://hsb-server:8080",
            "his-inpatient"          // 在 HSB 管理台注册的系统 ID
    );

    /** 患者入院，通知下游系统 */
    public void notifyAdmit(String patientId, String patientName,
                             String department, String bedNo) throws Exception {
        Map<String, Object> payload = Map.of(
                "patient_id",   patientId,
                "patient_name", patientName,
                "department",   department,
                "bed_no",       bedNo,
                "admit_time",   java.time.Instant.now().toString()
        );

        HsbClient.HsbResponse resp = hsbClient.sendJson("PATIENT_ADMIT", payload);
        System.out.println("消息已投递，ID: " + resp.messageId
                + "，命中路由: " + resp.matchedRoutes);
    }

    /** 发送 HL7 v2.x ADT^A01 */
    public void sendHl7Admit(String hl7Message) throws Exception {
        HsbClient.HsbResponse resp = hsbClient.sendHl7v2(hl7Message);
        System.out.println("HL7 消息已投递，ID: " + resp.messageId);
    }
}
```

### 7.3 MLLP（TCP）发送 HL7 v2.x

```java
package com.example.his;

import java.io.InputStream;
import java.io.OutputStream;
import java.net.Socket;
import java.nio.charset.StandardCharsets;

public class MllpSender {

    private static final byte MLLP_START  = 0x0B;
    private static final byte MLLP_END    = 0x1C;
    private static final byte MLLP_CR     = 0x0D;

    private final String host;
    private final int port;

    public MllpSender(String host, int port) {
        this.host = host;
        this.port = port; // HSB 默认 2575
    }

    /**
     * 发送 HL7 v2.x 报文并接收 ACK
     */
    public String send(String hl7Message) throws Exception {
        try (Socket socket = new Socket(host, port)) {
            socket.setSoTimeout(30_000);

            OutputStream out = socket.getOutputStream();
            InputStream  in  = socket.getInputStream();

            // 组装 MLLP 帧
            byte[] msgBytes = hl7Message.getBytes(StandardCharsets.UTF_8);
            byte[] frame = new byte[msgBytes.length + 3];
            frame[0] = MLLP_START;
            System.arraycopy(msgBytes, 0, frame, 1, msgBytes.length);
            frame[frame.length - 2] = MLLP_END;
            frame[frame.length - 1] = MLLP_CR;

            out.write(frame);
            out.flush();

            // 读取 ACK（简单读取，实际应解析 MLLP 帧边界）
            byte[] buf = new byte[4096];
            int len = in.read(buf);
            return len > 0 ? new String(buf, 1, len - 2, StandardCharsets.UTF_8) : "";
        }
    }

    public static void main(String[] args) throws Exception {
        MllpSender sender = new MllpSender("hsb-server", 2575);
        String hl7 = "MSH|^~\\&|HIS|HOSPITAL|HSB|NEXUS|20260512080000||ADT^A01|MSG001|P|2.5\r" +
                     "PID|1||P100001^^^HOSPITAL||张三\r";
        String ack = sender.send(hl7);
        System.out.println("ACK: " + ack);
    }
}
```

---

## 8. Rust 示例代码

使用 `reqwest`（异步 HTTP 客户端）。

### 8.1 `Cargo.toml` 依赖

```toml
[dependencies]
reqwest = { version = "0.12", features = ["json"] }
serde   = { version = "1",    features = ["derive"] }
serde_json = "1"
tokio   = { version = "1",    features = ["full"] }
uuid    = { version = "1",    features = ["v4"] }
```

### 8.2 HSB 客户端

```rust
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// HSB 入站响应
#[derive(Debug, Deserialize)]
pub struct HsbResponse {
    pub message_id: String,
    pub trace_id: String,
    pub protocol: String,
    pub matched_routes: Vec<String>,
}

/// HSB 错误响应
#[derive(Debug, Deserialize)]
pub struct HsbErrorResponse {
    pub error: String,
    pub code: String,
}

/// HSB 客户端
pub struct HsbClient {
    http: Client,
    base_url: String,
    source_system: String,
}

impl HsbClient {
    pub fn new(base_url: impl Into<String>, source_system: impl Into<String>) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            source_system: source_system.into(),
        }
    }

    /// 发送 JSON 消息
    pub async fn send_json<T: Serialize>(
        &self,
        message_type: &str,
        payload: &T,
    ) -> Result<HsbResponse, Box<dyn std::error::Error>> {
        self.send_json_with_target(message_type, payload, None).await
    }

    /// 发送 JSON 消息，指定目标系统
    pub async fn send_json_with_target<T: Serialize>(
        &self,
        message_type: &str,
        payload: &T,
        target_system: Option<&str>,
    ) -> Result<HsbResponse, Box<dyn std::error::Error>> {
        let trace_id = format!("hsb-{}", Uuid::new_v4());
        let url = format!("{}/api/messages/inbound", self.base_url);

        let mut req = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-HSB-Source-System", &self.source_system)
            .header("X-HSB-Message-Type", message_type)
            .header("X-HSB-Trace-Id", &trace_id)
            .json(payload);

        if let Some(target) = target_system {
            req = req.header("X-HSB-Target-System", target);
        }

        let resp = req.send().await?;
        let status = resp.status();

        if status == StatusCode::ACCEPTED {
            let body: HsbResponse = resp.json().await?;
            Ok(body)
        } else {
            let err: HsbErrorResponse = resp.json().await?;
            Err(format!("HSB error [{}]: {} ({})", status, err.error, err.code).into())
        }
    }

    /// 发送 HL7 v2.x 消息（HTTP 传输）
    pub async fn send_hl7v2(
        &self,
        hl7_message: &str,
    ) -> Result<HsbResponse, Box<dyn std::error::Error>> {
        let trace_id = format!("hsb-{}", Uuid::new_v4());
        let url = format!("{}/api/messages/inbound", self.base_url);

        let resp = self
            .http
            .post(&url)
            .header("Content-Type", "text/plain; charset=utf-8")
            .header("X-HSB-Source-System", &self.source_system)
            .header("X-HSB-Protocol", "HL7V2")
            .header("X-HSB-Trace-Id", &trace_id)
            .body(hl7_message.to_string())
            .send()
            .await?;

        let status = resp.status();
        if status == StatusCode::ACCEPTED {
            Ok(resp.json().await?)
        } else {
            let body = resp.text().await?;
            Err(format!("HSB error [{}]: {}", status, body).into())
        }
    }
}
```

### 8.3 业务使用示例

```rust
use serde::Serialize;

#[derive(Serialize)]
struct PatientAdmitEvent {
    patient_id: String,
    patient_name: String,
    department: String,
    bed_no: String,
    admit_time: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = HsbClient::new("http://hsb-server:8080", "his-inpatient");

    // 发送 JSON 消息
    let event = PatientAdmitEvent {
        patient_id: "P100001".into(),
        patient_name: "张三".into(),
        department: "内科".into(),
        bed_no: "12A".into(),
        admit_time: chrono::Utc::now().to_rfc3339(),
    };

    let resp = client.send_json("PATIENT_ADMIT", &event).await?;
    println!("消息已投递，ID: {}，命中路由: {:?}", resp.message_id, resp.matched_routes);

    // 发送 HL7 v2.x 消息
    let hl7 = "MSH|^~\\&|HIS|HOSPITAL|HSB|NEXUS|20260512080000||ADT^A01|MSG001|P|2.5\rPID|1||P100001";
    let resp = client.send_hl7v2(hl7).await?;
    println!("HL7 消息已投递，ID: {}", resp.message_id);

    Ok(())
}
```

### 8.4 MLLP TCP 发送

```rust
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const MLLP_START: u8 = 0x0B;
const MLLP_END: u8 = 0x1C;
const MLLP_CR: u8 = 0x0D;

async fn send_mllp(host: &str, port: u16, hl7: &str)
    -> Result<String, Box<dyn std::error::Error>>
{
    let mut stream = TcpStream::connect(format!("{host}:{port}")).await?;

    // 组装 MLLP 帧
    let msg_bytes = hl7.as_bytes();
    let mut frame = Vec::with_capacity(msg_bytes.len() + 3);
    frame.push(MLLP_START);
    frame.extend_from_slice(msg_bytes);
    frame.push(MLLP_END);
    frame.push(MLLP_CR);

    stream.write_all(&frame).await?;

    // 读取 ACK
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await?;
    let ack = String::from_utf8_lossy(&buf[1..n.saturating_sub(2)]).to_string();
    Ok(ack)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hl7 = "MSH|^~\\&|HIS|HOSPITAL|HSB|NEXUS|20260512080000||ADT^A01|MSG001|P|2.5\r\
               PID|1||P100001^^^HOSPITAL||张三\r";
    let ack = send_mllp("hsb-server", 2575, hl7).await?;
    println!("ACK: {}", ack);
    Ok(())
}
```

---

## 9. Python 示例代码

使用标准库 `urllib` 或第三方 `requests`（推荐），无需复杂依赖。

### 9.1 HSB 客户端

```python
"""HSB 客户端封装"""
import json
import socket
import struct
import uuid
from datetime import datetime, timezone
from typing import Any, Optional

import requests
from requests.adapters import HTTPAdapter, Retry


class HsbClient:
    """
    HSB HTTP 客户端。
    线程安全，推荐以单例方式使用。
    """

    def __init__(self, base_url: str, source_system: str, timeout: int = 30):
        self.base_url = base_url.rstrip("/")
        self.source_system = source_system
        self.timeout = timeout
        self._session = self._build_session()

    def _build_session(self) -> requests.Session:
        session = requests.Session()
        retry = Retry(
            total=3,
            backoff_factor=0.5,
            status_forcelist=[500, 502, 503, 504],
            allowed_methods=["POST"],
        )
        adapter = HTTPAdapter(max_retries=retry)
        session.mount("http://", adapter)
        session.mount("https://", adapter)
        return session

    def send_json(
        self,
        message_type: str,
        payload: dict[str, Any],
        target_system: Optional[str] = None,
        correlation_id: Optional[str] = None,
    ) -> dict:
        """
        发送 JSON 消息到 HSB。

        Args:
            message_type: 消息类型标识，如 "ORDER_CREATE"
            payload: 业务报文字典
            target_system: 可选，指定目标系统 ID
            correlation_id: 可选，关联 ID（用于请求响应配对）

        Returns:
            HSB 响应字典，含 message_id、trace_id 等字段

        Raises:
            requests.HTTPError: HTTP 非 202 响应
        """
        trace_id = f"hsb-{uuid.uuid4()}"
        headers = {
            "Content-Type": "application/json",
            "X-HSB-Source-System": self.source_system,
            "X-HSB-Message-Type": message_type,
            "X-HSB-Trace-Id": trace_id,
        }
        if target_system:
            headers["X-HSB-Target-System"] = target_system
        if correlation_id:
            headers["X-HSB-Correlation-Id"] = correlation_id

        resp = self._session.post(
            f"{self.base_url}/api/messages/inbound",
            json=payload,
            headers=headers,
            timeout=self.timeout,
        )
        resp.raise_for_status()
        return resp.json()

    def send_hl7v2(self, hl7_message: str) -> dict:
        """
        通过 HTTP 发送 HL7 v2.x 消息。

        Args:
            hl7_message: 原始 HL7 报文字符串（以 MSH| 开头）

        Returns:
            HSB 响应字典
        """
        trace_id = f"hsb-{uuid.uuid4()}"
        headers = {
            "Content-Type": "text/plain; charset=utf-8",
            "X-HSB-Source-System": self.source_system,
            "X-HSB-Protocol": "HL7V2",
            "X-HSB-Trace-Id": trace_id,
        }
        resp = self._session.post(
            f"{self.base_url}/api/messages/inbound",
            data=hl7_message.encode("utf-8"),
            headers=headers,
            timeout=self.timeout,
        )
        resp.raise_for_status()
        return resp.json()

    def send_fhir(self, fhir_resource: dict) -> dict:
        """发送 FHIR R5 资源"""
        trace_id = f"hsb-{uuid.uuid4()}"
        headers = {
            "Content-Type": "application/fhir+json",
            "X-HSB-Source-System": self.source_system,
            "X-HSB-Trace-Id": trace_id,
        }
        resp = self._session.post(
            f"{self.base_url}/api/messages/inbound",
            json=fhir_resource,
            headers=headers,
            timeout=self.timeout,
        )
        resp.raise_for_status()
        return resp.json()

    def health_check(self) -> bool:
        """检查 HSB 是否可用"""
        try:
            resp = self._session.get(
                f"{self.base_url}/health", timeout=5
            )
            return resp.status_code == 200
        except Exception:
            return False


class MllpClient:
    """MLLP TCP 客户端，用于 HL7 v2.x 传统 MLLP 接入"""

    MLLP_START = b"\x0b"
    MLLP_END   = b"\x1c\x0d"

    def __init__(self, host: str, port: int = 2575, timeout: int = 30):
        self.host = host
        self.port = port
        self.timeout = timeout

    def send(self, hl7_message: str) -> str:
        """
        发送 HL7 v2.x 报文并返回 ACK 字符串。
        """
        frame = self.MLLP_START + hl7_message.encode("utf-8") + self.MLLP_END

        with socket.create_connection((self.host, self.port), timeout=self.timeout) as sock:
            sock.sendall(frame)
            # 读取 ACK
            buf = b""
            while True:
                chunk = sock.recv(4096)
                if not chunk:
                    break
                buf += chunk
                if self.MLLP_END in buf:
                    break

        # 去掉 MLLP 控制字符，返回 HL7 ACK 文本
        ack = buf.lstrip(self.MLLP_START).rstrip(b"\x1c\x0d")
        return ack.decode("utf-8", errors="replace")
```

### 9.2 业务使用示例

```python
from hsb_client import HsbClient, MllpClient
from datetime import datetime, timezone

# --- 初始化客户端（建议全局单例）---
client = HsbClient(
    base_url="http://hsb-server:8080",
    source_system="his-inpatient",   # HSB 管理台注册的系统 ID
)


# --- 示例1：发送患者入院事件（JSON）---
def notify_patient_admit(patient_id: str, patient_name: str,
                          department: str, bed_no: str) -> None:
    payload = {
        "patient_id":   patient_id,
        "patient_name": patient_name,
        "department":   department,
        "bed_no":       bed_no,
        "admit_time":   datetime.now(timezone.utc).isoformat(),
    }
    result = client.send_json("PATIENT_ADMIT", payload)
    print(f"消息已投递，ID: {result['message_id']}，"
          f"命中路由: {result['matched_routes']}")


# --- 示例2：发送检验申请（指定目标系统）---
def send_lab_order(order_data: dict) -> None:
    result = client.send_json(
        "LAB_ORDER_CREATE",
        order_data,
        target_system="lis-main",     # 可选，不填则由路由规则决定
    )
    print(f"检验申请已发送，TraceId: {result['trace_id']}")


# --- 示例3：发送 FHIR Observation 资源 ---
def send_fhir_observation(patient_id: str, value: float, unit: str) -> None:
    fhir_resource = {
        "resourceType": "Observation",
        "status": "final",
        "subject": {"reference": f"Patient/{patient_id}"},
        "valueQuantity": {"value": value, "unit": unit},
    }
    result = client.send_fhir(fhir_resource)
    print(f"FHIR 资源已投递，ID: {result['message_id']}")


# --- 示例4：MLLP TCP 发送 HL7 v2.x ---
def send_hl7_via_mllp(patient_id: str) -> None:
    mllp = MllpClient("hsb-server", port=2575)
    hl7 = (
        f"MSH|^~\\&|HIS|HOSPITAL|HSB|NEXUS|"
        f"{datetime.now().strftime('%Y%m%d%H%M%S')}||ADT^A01|MSG001|P|2.5\r"
        f"PID|1||{patient_id}^^^HOSPITAL||张三\r"
    )
    ack = mllp.send(hl7)
    print(f"ACK: {ack}")


if __name__ == "__main__":
    # 测试连通性
    if client.health_check():
        print("HSB 连接正常")
        notify_patient_admit("P100001", "张三", "内科", "12A")
    else:
        print("HSB 不可用，请检查服务状态")
```

### 9.3 管理 API 操作示例（Python）

```python
import requests

HSB_ADMIN = "http://hsb-server:8081/api/v1"

# 注册系统
def register_system(org_id: str, name: str, system_type: str) -> str:
    resp = requests.post(f"{HSB_ADMIN}/systems", json={
        "organization_id": org_id,
        "name": name,
        "system_type": system_type,
    })
    resp.raise_for_status()
    return resp.json()["id"]

# 查询历史消息
def query_messages(source_system: str, page: int = 1, page_size: int = 20) -> list:
    resp = requests.get(f"{HSB_ADMIN}/messages", params={
        "source_system": source_system,
        "page": page,
        "page_size": page_size,
    })
    resp.raise_for_status()
    return resp.json()["items"]

# 查询死信队列并重放
def replay_dlq_messages() -> None:
    dlq_resp = requests.get(f"{HSB_ADMIN}/dlq").json()
    for msg in dlq_resp.get("items", []):
        print(f"重放死信消息: {msg['id']}")
        requests.post(f"{HSB_ADMIN}/dlq/{msg['id']}/reprocess").raise_for_status()

# 查看消息链路追踪
def trace_message(message_id: str) -> dict:
    resp = requests.get(f"{HSB_ADMIN}/audit/trace/{message_id}")
    resp.raise_for_status()
    return resp.json()
```

---

## 10. Golang 示例代码

以下示例使用 Go 标准库 `net/http`，无需额外依赖，适合直接集成到 HIS、LIS、PACS 等外围系统服务中。

### 10.1 HSB 客户端

```go
package hsb

import (
    "bytes"
    "context"
    "encoding/json"
    "fmt"
    "io"
    "net/http"
    "strings"
    "time"
)

// Client 是 HSB HTTP 客户端，推荐在业务服务中复用单例。
type Client struct {
    BaseURL      string
    SourceSystem string
    HTTPClient   *http.Client
}

// Response 对应 HSB /api/messages/inbound 成功响应。
type Response struct {
    MessageID     string     `json:"message_id"`
    TraceID       string     `json:"trace_id"`
    Protocol      string     `json:"protocol"`
    MatchedRoutes []string   `json:"matched_routes"`
    Deliveries    []Delivery `json:"deliveries"`
}

type Delivery struct {
    RouteID    string  `json:"route_id"`
    Target     string  `json:"target"`
    Success    bool    `json:"success"`
    DurationMS uint64  `json:"duration_ms"`
    Error      *string `json:"error"`
}

type ErrorResponse struct {
    Error string `json:"error"`
    Code  string `json:"code"`
}

func NewClient(baseURL, sourceSystem string) *Client {
    return &Client{
        BaseURL:      strings.TrimRight(baseURL, "/"),
        SourceSystem: sourceSystem,
        HTTPClient: &http.Client{
            Timeout: 30 * time.Second,
        },
    }
}

// SendJSON 发送 HTTP/JSON 消息。
func (c *Client) SendJSON(ctx context.Context, messageType string, payload any) (*Response, error) {
    return c.SendJSONWithTarget(ctx, messageType, payload, "")
}

// SendJSONWithTarget 发送 HTTP/JSON 消息，并可指定目标系统。
func (c *Client) SendJSONWithTarget(ctx context.Context, messageType string, payload any, targetSystem string) (*Response, error) {
    body, err := json.Marshal(payload)
    if err != nil {
        return nil, fmt.Errorf("marshal payload: %w", err)
    }

    req, err := http.NewRequestWithContext(ctx, http.MethodPost, c.BaseURL+"/api/messages/inbound", bytes.NewReader(body))
    if err != nil {
        return nil, err
    }

    traceID := fmt.Sprintf("hsb-%d", time.Now().UnixNano())
    req.Header.Set("Content-Type", "application/json")
    req.Header.Set("X-HSB-Source-System", c.SourceSystem)
    req.Header.Set("X-HSB-Message-Type", messageType)
    req.Header.Set("X-HSB-Trace-Id", traceID)
    if targetSystem != "" {
        req.Header.Set("X-HSB-Target-System", targetSystem)
    }

    return c.do(req)
}

// SendHL7V2 通过 HTTP 入站接口发送 HL7 v2.x 原始报文。
func (c *Client) SendHL7V2(ctx context.Context, hl7Message string) (*Response, error) {
    req, err := http.NewRequestWithContext(ctx, http.MethodPost, c.BaseURL+"/api/messages/inbound", strings.NewReader(hl7Message))
    if err != nil {
        return nil, err
    }

    traceID := fmt.Sprintf("hsb-%d", time.Now().UnixNano())
    req.Header.Set("Content-Type", "text/plain; charset=utf-8")
    req.Header.Set("X-HSB-Source-System", c.SourceSystem)
    req.Header.Set("X-HSB-Protocol", "HL7V2")
    req.Header.Set("X-HSB-Trace-Id", traceID)

    return c.do(req)
}

// SendFHIR 发送 FHIR R5 JSON 资源。
func (c *Client) SendFHIR(ctx context.Context, resource any) (*Response, error) {
    body, err := json.Marshal(resource)
    if err != nil {
        return nil, fmt.Errorf("marshal fhir resource: %w", err)
    }

    req, err := http.NewRequestWithContext(ctx, http.MethodPost, c.BaseURL+"/api/messages/inbound", bytes.NewReader(body))
    if err != nil {
        return nil, err
    }

    traceID := fmt.Sprintf("hsb-%d", time.Now().UnixNano())
    req.Header.Set("Content-Type", "application/fhir+json")
    req.Header.Set("X-HSB-Source-System", c.SourceSystem)
    req.Header.Set("X-HSB-Trace-Id", traceID)

    return c.do(req)
}

func (c *Client) do(req *http.Request) (*Response, error) {
    resp, err := c.HTTPClient.Do(req)
    if err != nil {
        return nil, err
    }
    defer resp.Body.Close()

    body, err := io.ReadAll(resp.Body)
    if err != nil {
        return nil, err
    }

    if resp.StatusCode != http.StatusAccepted {
        var errResp ErrorResponse
        if json.Unmarshal(body, &errResp) == nil && errResp.Code != "" {
            return nil, fmt.Errorf("hsb error [%d]: %s (%s)", resp.StatusCode, errResp.Error, errResp.Code)
        }
        return nil, fmt.Errorf("hsb error [%d]: %s", resp.StatusCode, string(body))
    }

    var result Response
    if err := json.Unmarshal(body, &result); err != nil {
        return nil, fmt.Errorf("decode hsb response: %w", err)
    }
    return &result, nil
}
```

### 10.2 业务使用示例

```go
package main

import (
    "context"
    "fmt"
    "log"
    "time"

    "example.com/his/hsb"
)

type PatientAdmitEvent struct {
    PatientID   string `json:"patient_id"`
    PatientName string `json:"patient_name"`
    Department  string `json:"department"`
    BedNo       string `json:"bed_no"`
    AdmitTime   string `json:"admit_time"`
}

func main() {
    client := hsb.NewClient("http://hsb-server:8080", "his-inpatient")

    ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
    defer cancel()

    event := PatientAdmitEvent{
        PatientID:   "P100001",
        PatientName: "张三",
        Department:  "内科",
        BedNo:       "12A",
        AdmitTime:   time.Now().Format(time.RFC3339),
    }

    resp, err := client.SendJSON(ctx, "PATIENT_ADMIT", event)
    if err != nil {
        log.Fatalf("发送 HSB 消息失败: %v", err)
    }
    fmt.Printf("消息已投递，ID: %s，命中路由: %v\n", resp.MessageID, resp.MatchedRoutes)

    hl7 := "MSH|^~\\&|HIS|HOSPITAL|HSB|NEXUS|20260512080000||ADT^A01|MSG001|P|2.5\r" +
        "PID|1||P100001^^^HOSPITAL||张三\r"
    hl7Resp, err := client.SendHL7V2(ctx, hl7)
    if err != nil {
        log.Fatalf("发送 HL7 消息失败: %v", err)
    }
    fmt.Printf("HL7 消息已投递，ID: %s\n", hl7Resp.MessageID)
}
```

### 10.3 MLLP TCP 发送

```go
package main

import (
    "bytes"
    "fmt"
    "net"
    "time"
)

const (
    mllpStart = byte(0x0B)
    mllpEnd   = byte(0x1C)
    mllpCR    = byte(0x0D)
)

func SendMLLP(address string, hl7Message string) (string, error) {
    conn, err := net.DialTimeout("tcp", address, 10*time.Second)
    if err != nil {
        return "", err
    }
    defer conn.Close()

    _ = conn.SetDeadline(time.Now().Add(30 * time.Second))

    frame := make([]byte, 0, len(hl7Message)+3)
    frame = append(frame, mllpStart)
    frame = append(frame, []byte(hl7Message)...)
    frame = append(frame, mllpEnd, mllpCR)

    if _, err := conn.Write(frame); err != nil {
        return "", err
    }

    buf := make([]byte, 4096)
    n, err := conn.Read(buf)
    if err != nil {
        return "", err
    }

    ack := bytes.TrimPrefix(buf[:n], []byte{mllpStart})
    ack = bytes.TrimSuffix(ack, []byte{mllpEnd, mllpCR})
    return string(ack), nil
}

func main() {
    hl7 := "MSH|^~\\&|HIS|HOSPITAL|HSB|NEXUS|20260512080000||ADT^A01|MSG001|P|2.5\r" +
        "PID|1||P100001^^^HOSPITAL||张三\r"

    ack, err := SendMLLP("hsb-server:2575", hl7)
    if err != nil {
        panic(err)
    }
    fmt.Println("ACK:", ack)
}
```

### 10.4 管理 API 操作示例（Golang）

```go
package main

import (
    "bytes"
    "context"
    "encoding/json"
    "fmt"
    "net/http"
    "time"
)

const adminBaseURL = "http://hsb-server:8081/api/v1"

func PostJSON(ctx context.Context, path string, payload any, result any) error {
    body, err := json.Marshal(payload)
    if err != nil {
        return err
    }

    req, err := http.NewRequestWithContext(ctx, http.MethodPost, adminBaseURL+path, bytes.NewReader(body))
    if err != nil {
        return err
    }
    req.Header.Set("Content-Type", "application/json")

    client := &http.Client{Timeout: 30 * time.Second}
    resp, err := client.Do(req)
    if err != nil {
        return err
    }
    defer resp.Body.Close()

    if resp.StatusCode < 200 || resp.StatusCode >= 300 {
        return fmt.Errorf("admin api status: %s", resp.Status)
    }

    if result != nil {
        return json.NewDecoder(resp.Body).Decode(result)
    }
    return nil
}

func RegisterSystem(ctx context.Context, organizationID string) (string, error) {
    payload := map[string]any{
        "organization_id": organizationID,
        "name":            "住院管理系统",
        "system_type":     "HIS",
        "description":     "住院 HIS，负责床位、入出转业务",
    }

    var result struct {
        ID string `json:"id"`
    }
    if err := PostJSON(ctx, "/systems", payload, &result); err != nil {
        return "", err
    }
    return result.ID, nil
}

func ReplayDLQMessage(ctx context.Context, messageID string) error {
    return PostJSON(ctx, "/dlq/"+messageID+"/reprocess", map[string]any{}, nil)
}
```

---

## 11. 错误码与排障

### 11.1 HTTP 状态码

| 状态码 | 含义 | 处理建议 |
|--------|------|---------|
| `202 Accepted` | 消息已接收并成功投递 | 正常 |
| `400 Bad Request` | 请求参数错误（缺少必填 Header、JSON 格式错误等）| 检查请求格式 |
| `404 Not Found` | 无匹配路由（`ROUTE_NOT_FOUND`）| 检查路由配置，确认 source_system 和 message_type 是否与路由规则一致 |
| `500 Internal Server Error` | 服务内部错误 | 联系 HSB 管理员，查看 `/api/v1/audit/trace/{message_id}` |
| `503 Service Unavailable` | 服务未就绪 | 等待服务启动，或检查依赖（PostgreSQL/RabbitMQ）|

### 11.2 常见错误码

| error_code | 含义 | 解决方案 |
|------------|------|---------|
| `MISSING_CONFIG` | 缺少必填 Header，通常是 `X-HSB-Source-System` | 在请求头中添加 `X-HSB-Source-System` |
| `ROUTE_NOT_FOUND` | 消息无匹配路由 | 在管理台检查路由规则，确认来源系统 ID、消息类型是否正确 |
| `DELIVERY_FAILED` | 所有路由投递均失败 | 检查目标端点地址和连通性，查看端点健康状态 |
| `INVALID_FIELD` | 字段格式错误，如 `X-HSB-Protocol` 不合法 | 参照协议标识表修正值 |
| `ADAPTER_PARSE_ERROR` | 协议解析失败（如 HL7 格式错误）| 检查报文格式是否符合对应协议规范 |

### 11.3 排障步骤

```
1. 检查 HSB 服务状态
   GET http://hsb-server:8080/health

2. 用 message_id 查询消息状态和链路
   GET http://hsb-server:8081/api/v1/audit/trace/{message_id}

3. 检查端点健康状态
   GET http://hsb-server:8081/api/v1/endpoints/{endpoint_id}/health

4. 查看死信队列
   GET http://hsb-server:8081/api/v1/dlq

5. 查看熔断器状态
   GET http://hsb-server:8081/api/v1/circuit-breakers
```

---

## 12. 最佳实践

### 12.1 消息发送

- **幂等性**：为每次请求设置唯一的 `X-HSB-Trace-Id`，便于追踪和排查
- **重试策略**：业务层不要无限重试，建议最多 3 次，间隔指数退避
- **批量场景**：高频消息建议通过 MQ（RabbitMQ/Kafka）接入，而非 HTTP 轮询
- **超时设置**：HTTP 请求超时建议设为 30~60 秒，MLLP 连接超时 30 秒

### 12.2 系统注册

- `source_system` ID 建议使用**系统缩写 + 环境**格式，如 `his-inpatient-prod`
- 不同环境（开发/测试/生产）应注册不同的系统 ID，避免路由混淆
- 端点地址变更后需及时在管理台更新，并执行健康检查验证

### 12.3 消息类型设计

- `X-HSB-Message-Type` 建议使用 `动词_名词` 大写格式，如 `PATIENT_ADMIT`、`ORDER_CREATE`、`REPORT_COMPLETED`
- 与 HSB 管理员提前约定 Topic 命名规范，格式建议：`<domain>.<service>.<action>.<version>`

### 12.4 安全

- 生产环境应启用 TLS（端点配置 `require_tls: true`）
- 系统 ID 和端点 Token 不要硬编码在代码中，使用配置文件或密钥管理服务
- 定期检查熔断器状态，异常时及时通知运维处置

---

> 如有问题，请联系 HSB 平台管理员，或通过管理台 `/api/v1/audit/trace/{message_id}` 查看完整消息链路。
