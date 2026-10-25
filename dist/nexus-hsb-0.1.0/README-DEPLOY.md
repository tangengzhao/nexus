# Nexus HSB v0.1.0 生产部署包

本目录为完整的生产发布包。**目标服务器只需要：Linux；启用 PostgreSQL 持久化时另需 PostgreSQL 14+，不需要 Rust/Cargo 工具链。**

## 一、目录说明

| 文件/目录 | 用途 |
|---|---|
| hsb-server | 服务二进制（内嵌 Web 管理台 /ui/ 与管理 API） |
| config/hsb.toml | 配置文件（端口、持久化、消息队列、SSO 等） |
| deploy/k8s | Kubernetes 部署清单 |
| Dockerfile / docker-start.sh | 容器方式部署（可选） |
| start.sh / stop.sh | 裸机启停脚本（PID 与日志管理） |
| SHA256SUMS | 全部文件校验和（sha256sum -c SHA256SUMS 验证） |

## 二、裸机部署（推荐）

```bash
# 1. 校验完整性
sha256sum -c SHA256SUMS

# 2. 按需修改配置（端口 / 数据库 / 消息队列 / SSO）
vim config/hsb.toml

# 3. 检查配置合法性
./hsb-server check --config config/hsb.toml

# 4. 前台启动验证（默认 HTTP 8080 / Admin 8081 / TCP-MLLP 2575 / gRPC 10051）
./hsb-server start --config config/hsb.toml

# 5. 生产运行（后台，带 PID 与日志管理）
./start.sh
```

启动后访问：
- Web 管理台: http://<host>:8080/ui/（消息录入、端点/路由管理、监控）
- 管理 API: http://<host>:8080/api/v1/
- 业务入站: POST http://<host>:8080/api/messages/inbound

## 三、版本升级

```bash
# 用新版本的发布包替换旧目录（保留 config/、data/、logs/）后：
./hsb-server check --config config/hsb.toml   # 校验配置
./stop.sh && ./start.sh                       # 重启生效
```

说明：服务启动时会自动从持久层恢复未完成消息并重放，重启不丢消息。

## 四、Docker 部署（可选）

```bash
# 服务启动（config/data/logs 挂载到宿主机）
./docker-start.sh
```
