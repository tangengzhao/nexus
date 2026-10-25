#!/usr/bin/env bash
# =============================================================================
# docker-start.sh — 构建并启动 HSB Server 容器
#
# 用法：
#   ./docker-start.sh               # 普通启动（自动选择构建模式）
#   HSB_FORCE_BUILD=1 ./docker-start.sh   # 强制重新构建镜像
#   HSB_FULL_BUILD=1  ./docker-start.sh   # 强制使用完整 Rust 源码构建
#
# 可配置环境变量：
#   HSB_IMAGE          镜像名称（默认 hsb-server:latest）
#   HSB_CONTAINER      容器名称（默认 hsb-server）
#   HSB_CONFIG_DIR     宿主机配置目录（默认 <脚本目录>/config）
#   HSB_DATA_DIR       宿主机数据目录（默认 <脚本目录>/data）
#   HSB_LOGS_DIR       宿主机日志目录（默认 <脚本目录>/logs）
#   HSB_HTTP_PORT      HTTP 主端口（默认 8080）
#   HSB_ADMIN_PORT     HTTP 管理端口（默认 8081）
#   HSB_TCP_PORT       HL7 TCP/MLLP 端口（默认 2575）
#   HSB_GRPC_PORT      gRPC 端口（默认 10051）
#   HSB_EXTRA_ARGS     传给 docker run 的额外参数（默认空）
# =============================================================================

set -euo pipefail

# ── 参数默认值 ──────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

IMAGE_NAME="${HSB_IMAGE:-hsb-server:latest}"
CONTAINER_NAME="${HSB_CONTAINER:-hsb-server}"

HOST_CONFIG_DIR="${HSB_CONFIG_DIR:-$SCRIPT_DIR/config}"
HOST_DATA_DIR="${HSB_DATA_DIR:-$SCRIPT_DIR/data}"
HOST_LOGS_DIR="${HSB_LOGS_DIR:-$SCRIPT_DIR/logs}"

HSB_HTTP_PORT="${HSB_HTTP_PORT:-8080}"
HSB_ADMIN_PORT="${HSB_ADMIN_PORT:-8081}"
HSB_TCP_PORT="${HSB_TCP_PORT:-2575}"
HSB_GRPC_PORT="${HSB_GRPC_PORT:-10051}"

# ── 检查 Docker 是否可用 ────────────────────────────────────────────────────
if ! command -v docker &>/dev/null; then
  echo "错误：未找到 docker 命令，请先安装 Docker Engine。" >&2
  exit 1
fi

# ── 创建宿主机挂载目录 ──────────────────────────────────────────────────────
mkdir -p "$HOST_CONFIG_DIR" "$HOST_DATA_DIR" "$HOST_LOGS_DIR"
echo "目录确认: config=$HOST_CONFIG_DIR  data=$HOST_DATA_DIR  logs=$HOST_LOGS_DIR"

# 如果 config 目录中没有 hsb.toml，自动复制默认配置
if [[ ! -f "$HOST_CONFIG_DIR/hsb.toml" ]]; then
  if [[ -f "$SCRIPT_DIR/config/hsb.toml" ]]; then
    cp "$SCRIPT_DIR/config/hsb.toml" "$HOST_CONFIG_DIR/hsb.toml"
    echo "已复制默认配置到 $HOST_CONFIG_DIR/hsb.toml"
  else
    echo "警告：$HOST_CONFIG_DIR/hsb.toml 不存在，请在启动前放入配置文件。" >&2
  fi
fi

# ── 决定是否需要（重新）构建镜像 ──────────────────────────────────────────
needs_build=false

if [[ "${HSB_FORCE_BUILD:-0}" == "1" ]]; then
  needs_build=true
  echo "HSB_FORCE_BUILD=1，强制重新构建镜像..."
elif ! docker image inspect "$IMAGE_NAME" &>/dev/null; then
  needs_build=true
  echo "镜像 $IMAGE_NAME 不存在，开始构建..."
fi

if $needs_build; then
  BUILDMODE="full"
  # 如果宿主机存在预编译 release 二进制且未要求完整构建，优先使用 prebuilt 模式
  if [[ "${HSB_FULL_BUILD:-0}" != "1" ]] && \
     [[ -x "$SCRIPT_DIR/target/release/hsb-server" ]]; then
    BUILDMODE="prebuilt"
    echo "检测到预编译二进制，使用 BUILDMODE=prebuilt（快速打包）"
  else
    echo "使用 BUILDMODE=full（从 Rust 源码完整编译，耗时较长）"
  fi

  docker build \
    --build-arg "BUILDMODE=$BUILDMODE" \
    -t "$IMAGE_NAME" \
    "$SCRIPT_DIR"

  echo "镜像构建完成：$IMAGE_NAME"
fi

# ── 停止并移除旧容器 ────────────────────────────────────────────────────────
if docker ps -a --format '{{.Names}}' | grep -qx "$CONTAINER_NAME"; then
  echo "停止并移除已有容器：$CONTAINER_NAME"
  docker rm -f "$CONTAINER_NAME" &>/dev/null
fi

# ── 启动容器 ────────────────────────────────────────────────────────────────
echo "启动容器 $CONTAINER_NAME ..."

docker run -d \
  --name "$CONTAINER_NAME" \
  --restart unless-stopped \
  -p "${HSB_HTTP_PORT}:8080" \
  -p "${HSB_ADMIN_PORT}:8081" \
  -p "${HSB_TCP_PORT}:2575" \
  -p "${HSB_GRPC_PORT}:10051" \
  -v "${HOST_CONFIG_DIR}:/app/config:ro" \
  -v "${HOST_DATA_DIR}:/app/data" \
  -v "${HOST_LOGS_DIR}:/app/logs" \
  ${HSB_EXTRA_ARGS:-} \
  "$IMAGE_NAME"

echo ""
echo "HSB Server 已启动："
echo "  容器名：$CONTAINER_NAME"
echo "  HTTP    http://localhost:${HSB_HTTP_PORT}"
echo "  管理UI  http://localhost:${HSB_ADMIN_PORT}"
echo "  HL7 TCP 0.0.0.0:${HSB_TCP_PORT}"
echo "  gRPC    0.0.0.0:${HSB_GRPC_PORT}"
echo ""
echo "查看日志：docker logs -f $CONTAINER_NAME"
echo "停止容器：docker stop $CONTAINER_NAME"
