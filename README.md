# Nexus - Hospital Service Bus (HSB)

医院服务总线 - 一个用 Rust 编写的企业级医疗信息集成平台。

## 🏗️ 架构概述

Nexus HSB 采用六层架构设计，提供完整的医疗信息交换能力：

```
┌─────────────────────────────────────────────────────────────┐
│                    运维管理层 (hsb-admin)                     │
│         Admin API / 配置管理 / 监控告警 / 日志可视化            │
├─────────────────────────────────────────────────────────────┤
│                    数据与审计层 (hsb-audit)                    │
│         消息追踪 / 审计日志 / 合规记录 / 指标收集               │
├─────────────────────────────────────────────────────────────┤
│                    可靠性层 (hsb-reliability)                  │
│         消息队列 / 重试机制 / 死信队列 / 熔断器                  │
├─────────────────────────────────────────────────────────────┤
│                    消息核心层 (hsb-engine)                     │
│         路由引擎 / 消息转换 / 处理管道 / 组件注册               │
├─────────────────────────────────────────────────────────────┤
│                    协议适配层 (hsb-adapter-*)                  │
│         HL7 v2.x / FHIR R5 / DICOM / SOAP                     │
├─────────────────────────────────────────────────────────────┤
│                    接入层 (hsb-transport-*)                    │
│         HTTP/HTTPS / TCP/MLLP / gRPC / RabbitMQ               │
└─────────────────────────────────────────────────────────────┘
```
sequenceDiagram
    autonumber
    participant Client as 上游系统/调用方
    participant HTTP as HTTP入站接口
    participant Runtime as MessageIngressRuntime
    participant Adapter as ProtocolAdapter
    participant Msg as Canonical Message
    participant Pipe as ProcessingPipeline
    participant Store as MessageStore
    participant Router as Router
    participant Route as Route规则
    participant Disp as Dispatcher
    participant Endpoints as EndpointRegistry
    participant Endpoint as Endpoint配置
    participant Idem as IdempotencyStore
    participant Transport as Transport
    participant Target as 下游系统

    Client->>HTTP: 发送原始报文
    HTTP->>Runtime: handle_http_inbound(headers, body)

    alt HTTP/JSON或通用协议
        Runtime->>Msg: build_generic_http_message(...)
    else HL7/FHIR/DICOM/SOAP
        Runtime->>Adapter: parse(raw)
        Adapter-->>Runtime: Message
        Runtime->>Msg: 补齐source_system/protocol/message_type等
    end

    Runtime->>Store: save_message(Received)
    Runtime->>Pipe: execute(MessageContext)
    Pipe-->>Runtime: 处理后的MessageContext
    Runtime->>Store: save_message(Processing/Routing)

    Runtime->>Router: find_routes(message)
    Router->>Route: matches(source_match + conditions)
    Route-->>Router: 返回匹配路由集合
    Router-->>Runtime: routes

    alt 无匹配路由
        Runtime->>Store: save_message(Failed)
        Runtime-->>HTTP: RouteNotFound
    else 有匹配路由
        loop 每条匹配路由
            Runtime->>Disp: dispatch(ctx, route)
            Disp->>Endpoints: get(target.endpoint_id)
            Endpoints-->>Disp: EndpointInfo
            Disp->>Endpoint: 读取protocol/address/auth

            alt delivery_mode = exactly_once
                Disp->>Idem: check_and_mark(route:endpoint:correlation)
                alt 已处理过
                    Idem-->>Disp: false
                    Disp-->>Runtime: success(去重，不重复下发)
                else 新请求
                    Idem-->>Disp: true
                    Disp->>Transport: send(target address, body)
                    Transport->>Target: 下发消息
                    Target-->>Transport: 响应/ACK
                    Transport-->>Disp: DispatchResult
                end
            else at_most_once / at_least_once
                Disp->>Transport: send(target address, body)
                Transport->>Target: 下发消息
                Target-->>Transport: 响应/ACK
                Transport-->>Disp: DispatchResult
            end
        end

        alt 任一路由成功
            Runtime->>Store: save_message(Completed)
            Runtime-->>HTTP: ACCEPTED + matched_routes + deliveries
        else 全部失败
            Runtime->>Store: save_message(Failed)
            Runtime-->>HTTP: DeliveryFailed
        end
    end
## 📦 Crate 结构

| Crate | 描述 |
|-------|------|
| `hsb-common` | 公共类型、错误处理、工具函数 |
| `hsb-core` | 核心消息模型、路由规则、转换器 |
| `hsb-adapter-base` | 协议适配器基础 trait |
| `hsb-adapter-hl7` | HL7 v2.x 协议适配器 |
| `hsb-adapter-fhir` | FHIR R5 协议适配器 |
| `hsb-adapter-dicom` | DICOM 协议适配器 |
| `hsb-adapter-soap` | SOAP 1.1/1.2 协议适配器 |
| `hsb-transport-base` | 传输层基础 trait |
| `hsb-transport-http` | HTTP/HTTPS 传输 |
| `hsb-transport-tcp` | TCP/MLLP 传输 |
| `hsb-transport-mq` | 消息队列 (RabbitMQ) 传输 |
| `hsb-transport-grpc` | gRPC 传输 (rust-sso 集成) |
| `hsb-engine` | 路由引擎和处理管道 |
| `hsb-reliability` | 可靠性层 (队列、重试、DLQ) |
| `hsb-audit` | 审计和追踪 |
| `hsb-admin` | 管理 REST API |
| `hsb-server` | 主服务入口 |

## 🚀 快速开始

### 前置要求

- Rust 1.85+ (Edition 2024)
- PostgreSQL 14+
- RabbitMQ 3.12+

### 构建

```bash
cd nexus
cargo build --release
```

### 运行

```bash
# 生成默认配置
./target/release/hsb-server init --output config/hsb.toml

# 检查配置
./target/release/hsb-server check --config config/hsb.toml

# 启动服务
./target/release/hsb-server start --config config/hsb.toml
```

启动后可直接访问：

- 主入口管理台: `/ui/`，由 axum 主 HTTP 端口统一对外服务
- 同口管理 API: `/api/v1/...`
- 业务入站接口: `/api/messages/inbound`

说明：`http.admin_port` 上的独立 Admin API 仍然保留，便于兼容原有调用方式；当前推荐优先使用主 HTTP 端口上的统一出口。

### 命令行选项

```
HSB - Hospital Service Bus

Usage: hsb-server [OPTIONS] [COMMAND]

Commands:
  start    启动服务器
  check    检查配置
  version  显示版本信息
  init     生成默认配置
  help     Print this message or the help of the given subcommand(s)

Options:
  -c, --config <CONFIG>      配置文件路径 [default: config/hsb.toml]
  -l, --log-level <LEVEL>    日志级别 [default: info]
      --json-log             以 JSON 格式输出日志
  -h, --help                 Print help
  -V, --version              Print version
```

## 🔧 配置

详细配置请参考 [config/hsb.toml](config/hsb.toml)。

### 主要配置项

| 配置项 | 描述 | 默认值 |
|--------|------|--------|
| `server.max_concurrency` | 最大并发处理数 | 1000 |
| `http.port` | HTTP 服务端口 | 8080 |
| `http.admin_port` | Admin API 端口 | 8081 |
| `tcp.port` | TCP/MLLP 端口 | 2575 |
| `grpc.port` | gRPC 端口 | 50051 |
| `reliability.max_retries` | 最大重试次数 | 3 |
| `audit.retention_days` | 审计日志保留天数 | 90 |

## 📡 API 端点

### Admin API

管理 API 既可通过独立 `http.admin_port` 访问，也可通过主 HTTP 端口下的统一出口访问。

| 端点 | 方法 | 描述 |
|------|------|------|
| `/api/v1/health` | GET | 健康检查 |
| `/api/v1/status` | GET | 系统状态 |
| `/api/v1/routes` | GET/POST | 路由管理 |
| `/api/v1/endpoints` | GET/POST | 端点管理 |
| `/api/v1/endpoints/:id` | GET/PUT/DELETE | 单个端点查询、更新、删除 |
| `/api/v1/endpoints/:id/versions` | GET | 端点版本历史 |
| `/api/v1/endpoints/:id/status` | GET/PUT | 端点运行状态 |
| `/api/v1/endpoints/:id/security` | PUT | 端点安全配置轮换 |
| `/api/v1/endpoints/:id/health` | GET | 端点健康视图 |
| `/api/v1/workflows` | GET/POST | 工作流定义查询、创建 |
| `/api/v1/workflows/:id` | GET/PUT/DELETE | 工作流定义查询、更新、删除 |
| `/api/v1/workflows/:id/start` | POST | 启动工作流实例 |
| `/api/v1/workflow-instances` | GET | 工作流实例列表 |
| `/api/v1/workflow-instances/:id` | GET | 工作流实例详情 |
| `/api/v1/workflow-instances/:id/pause` | POST | 暂停工作流实例 |
| `/api/v1/workflow-instances/:id/resume` | POST | 恢复工作流实例 |
| `/api/v1/workflow-instances/:id/cancel` | POST | 取消工作流实例 |
| `/api/v1/workflow-instances/:id/compensate` | POST | 触发工作流补偿 |
| `/api/v1/dlq` | GET | 死信队列 |
| `/api/v1/audit` | GET | 审计日志查询 |
| `/api/v1/metrics` | GET | 指标数据 |

工作流定义在启用 PostgreSQL 持久化时会保存在数据库中；管理台通过主 HTTP 端口的 /ui/ 页面直接调用上述 API。

### Endpoint 管理示例

Endpoint 现在支持完整的配置管理、版本管理、运行状态管理和安全配置轮换。管理面以 PostgreSQL 为事实来源，服务启动时会把已保存的 endpoint 自动回填到运行时注册表。

创建 endpoint：

```bash
curl -X POST http://hsb-admin:8081/api/v1/endpoints \
  -H 'Content-Type: application/json' \
  -d '{
    "id": "HIS_ENDPOINT_01",
    "name": "HIS 主接口",
    "description": "核心门诊接口",
    "system_type": "HIS",
    "protocol": "HTTP",
    "connection": {
      "host": "his-api.internal",
      "port": 8443,
      "path": "/api/v1/orders",
      "tls_enabled": true,
      "tls_cert_path": null,
      "connect_timeout_secs": 10,
      "read_timeout_secs": 30,
      "write_timeout_secs": 30,
      "pool_size": 16,
      "reconnect_interval_secs": 5,
      "keepalive_secs": 60
    },
    "auth": {
      "type": "basic",
      "username": "his_user",
      "password": "stored-in-secret-manager"
    },
    "security": {
      "secret_ref": "vault://hsb/endpoints/his-primary",
      "require_tls": true,
      "allow_insecure_skip_verify": false,
      "allowed_ip_ranges": ["10.20.0.0/16"],
      "mask_credentials_in_logs": true,
      "credential_expires_at": null,
      "credential_last_rotated_at": null
    },
    "created_by": "ops-user",
    "change_note": "initial create"
  }'
```

更新 endpoint 基础配置并产生新版本：

```bash
curl -X PUT http://hsb-admin:8081/api/v1/endpoints/HIS_ENDPOINT_01 \
  -H 'Content-Type: application/json' \
  -d '{
    "name": "HIS 主接口-切换",
    "enabled": false,
    "lifecycle_status": "DISABLED",
    "updated_by": "ops-user-2",
    "change_note": "disable for switch"
  }'
```

更新运行状态：

```bash
curl -X PUT http://hsb-admin:8081/api/v1/endpoints/HIS_ENDPOINT_01/status \
  -H 'Content-Type: application/json' \
  -d '{
    "healthy": false,
    "latency_ms": 150,
    "last_error": "upstream timeout",
    "circuit_state": "open",
    "consecutive_failures": 3
  }'
```

轮换安全配置：

```bash
curl -X PUT http://hsb-admin:8081/api/v1/endpoints/HIS_ENDPOINT_01/security \
  -H 'Content-Type: application/json' \
  -d '{
    "secret_ref": "vault://hsb/endpoints/his-primary-v2",
    "require_tls": true,
    "allowed_ip_ranges": ["10.20.0.0/16", "10.30.0.0/16"],
    "mask_credentials_in_logs": true,
    "rotated_by": "sec-user",
    "change_note": "rotate endpoint secret"
  }'
```

查看版本历史与健康状态：

```bash
curl http://hsb-admin:8081/api/v1/endpoints/HIS_ENDPOINT_01/versions
curl http://hsb-admin:8081/api/v1/endpoints/HIS_ENDPOINT_01/health
```

## 🔒 与 rust-sso 集成

Nexus HSB 可通过 gRPC 与 rust-sso 集成实现认证授权：

```toml
[sso]
enabled = true
endpoint = "${RUST_SSO_GRPC_ENDPOINT}"
web_base_url = "${RUST_SSO_WEB_BASE_URL}"
client_id = "hsb-service"
client_secret = "your-secret"
callback_url = "${HSB_SSO_CALLBACK_URL}"
scope = "openid profile email"
```

当浏览器访问主页时，未登录用户会先跳转到统一单点登录页；登录完成后，SSO 会回调到本项目的 /auth/callback，再回到主页。

## 📋 支持的协议

### HL7 v2.x

- 版本：2.1 - 2.10
- 消息类型：ADT、ORM、ORU、MDM 等
- 传输：TCP/MLLP

### FHIR R5

- 格式：JSON、XML
- Bundle 支持
- RESTful API

### DICOM

- DIMSE 服务：C-STORE、C-FIND、C-MOVE
- Modality Worklist
- Storage Commitment

### SOAP

- 版本：1.1、1.2
- WS-Security 支持
- WS-Addressing 支持

## 🛡️ 可靠性特性

- **消息持久化**：所有消息持久化存储
- **自动重试**：可配置的指数退避重试策略
- **死信队列**：失败消息自动进入 DLQ
- **熔断器**：防止级联故障
- **优先级队列**：支持消息优先级

## 📊 可观测性

- **分布式追踪**：OpenTelemetry 集成
- **指标收集**：Prometheus 兼容指标
- **审计日志**：完整的消息处理审计
- **敏感数据脱敏**：自动脱敏患者信息

## 📁 项目结构

```
nexus/
├── Cargo.toml              # Workspace 配置
├── README.md               # 本文档
├── config/
│   └── hsb.toml            # 默认配置
├── hsb-common/             # 公共模块
├── hsb-core/               # 核心模型
├── hsb-adapter-base/       # 适配器基础
├── hsb-adapter-hl7/        # HL7 适配器
├── hsb-adapter-fhir/       # FHIR 适配器
├── hsb-adapter-dicom/      # DICOM 适配器
├── hsb-adapter-soap/       # SOAP 适配器
├── hsb-transport-base/     # 传输层基础
├── hsb-transport-http/     # HTTP 传输
├── hsb-transport-tcp/      # TCP/MLLP 传输
├── hsb-transport-mq/       # MQ 传输
├── hsb-transport-grpc/     # gRPC 传输
├── hsb-engine/             # 路由引擎
├── hsb-reliability/        # 可靠性层
├── hsb-audit/              # 审计层
├── hsb-admin/              # Admin API
└── hsb-server/             # 主服务
```

## 🔨 开发

### 运行测试

```bash
cargo test --workspace
```

### 代码检查

```bash
cargo clippy --workspace
cargo fmt --check
```

### 生成文档

```bash
cargo doc --workspace --no-deps --open
```

## 📄 许可证

MIT License

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！
