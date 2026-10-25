# =============================================================================
# HSB Server Dockerfile
#
# 两种构建模式（通过 --build-arg BUILDMODE=<mode> 切换）：
#
#   full      （默认）从 Rust 源码完整编译，适合 CI/CD 和生产发布
#             docker build -t hsb-server:latest .
#
#   prebuilt  使用宿主机已编译的 release 二进制，适合本地快速打包验证
#             docker build --build-arg BUILDMODE=prebuilt -t hsb-server:latest .
#             前提：./target/release/hsb-server 已存在
#
# 端口映射（均在容器内监听 0.0.0.0）：
#   8080  HTTP 主入口
#   8081  HTTP 管理端口
#   2575  TCP / HL7 MLLP
#   10051 gRPC
#
# 关键 volume 挂载（由 docker-start.sh 自动传入）：
#   /app/config   配置文件目录（hsb.toml，只读）
#   /app/data     持久化数据（redb 嵌入式 KV，读写）
#   /app/logs     日志与 PID 文件（读写）
# =============================================================================

# 全局构建模式参数（必须在所有 FROM 之前声明）
ARG BUILDMODE=full

# -----------------------------------------------------------------------------
# Stage: build-full  ── 从源码编译（生产 / CI 使用）
# -----------------------------------------------------------------------------
FROM rust:latest AS build-full

WORKDIR /workspace

# 安装构建依赖：protoc（hsb-mock-endpoints 需要）+ cmake/libssl-dev/libsasl2-dev（rdkafka cmake-build 需要）
RUN apt-get update && apt-get install -y --no-install-recommends \
        protobuf-compiler \
        cmake \
        libssl-dev \
        libsasl2-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

# 先复制 Cargo 清单和 lock 文件以利用 Docker 层缓存
COPY Cargo.toml Cargo.lock ./
COPY hsb-common/Cargo.toml        hsb-common/Cargo.toml
COPY hsb-core/Cargo.toml          hsb-core/Cargo.toml
COPY hsb-adapter/Cargo.toml       hsb-adapter/Cargo.toml
COPY hsb-transport/Cargo.toml     hsb-transport/Cargo.toml
COPY hsb-transport/build.rs       hsb-transport/build.rs
COPY hsb-admin/Cargo.toml         hsb-admin/Cargo.toml
COPY hsb-server/Cargo.toml        hsb-server/Cargo.toml
COPY hsb-mock-endpoints/Cargo.toml  hsb-mock-endpoints/Cargo.toml
COPY hsb-mock-endpoints/build.rs    hsb-mock-endpoints/build.rs
COPY hsb-mock-endpoints/proto/      hsb-mock-endpoints/proto/

# 创建最小 stub 源文件，预编译全部依赖（提升增量构建速度）
RUN set -e; \
    for crate in hsb-common hsb-core hsb-adapter hsb-transport hsb-admin; do \
        mkdir -p $crate/src && printf '' > $crate/src/lib.rs; \
    done; \
    mkdir -p hsb-server/src && printf 'fn main(){}' > hsb-server/src/main.rs; \
    mkdir -p hsb-mock-endpoints/src && \
        printf '' > hsb-mock-endpoints/src/lib.rs && \
        printf 'fn main(){}' > hsb-mock-endpoints/src/main.rs && \
        mkdir -p hsb-mock-endpoints/src/bin

# 预编译依赖（失败时忽略，源文件替换后会重建）
RUN cargo build --release --bin hsb-server 2>/dev/null || true; \
    rm -f target/release/hsb-server target/release/deps/hsb_server*

# 复制完整源代码，重新编译最终二进制
COPY . .
# touch 所有 .rs 文件，确保时间戳新于 stub 编译产物，避免 Cargo 误用旧 artifacts
RUN find . -name "*.rs" -exec touch {} +
RUN cargo build --release --bin hsb-server

RUN mkdir -p /out && cp target/release/hsb-server /out/hsb-server

# -----------------------------------------------------------------------------
# Stage: build-prebuilt  ── 直接复制宿主机已编译的二进制（本地快速验证）
# -----------------------------------------------------------------------------
FROM debian:trixie-slim AS build-prebuilt

RUN mkdir -p /out
# 从构建上下文复制预编译二进制（需要 .dockerignore 中保留 target/release/hsb-server）
COPY target/release/hsb-server /out/hsb-server

# -----------------------------------------------------------------------------
# Stage: binary  ── 根据 BUILDMODE 选择二进制来源
# -----------------------------------------------------------------------------
# shellcheck 不支持此语法，但 Docker BuildKit 支持
# hadolint ignore=DL3006
FROM build-${BUILDMODE} AS binary

# -----------------------------------------------------------------------------
# Stage: runtime  ── 最小化运行时镜像
# -----------------------------------------------------------------------------
FROM debian:trixie-slim AS runtime

# 安装运行时所需动态库（由 ldd 分析确认）
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
        libgcc-s1 \
        libzstd1 \
        curl \
    && rm -rf /var/lib/apt/lists/*

# 创建非 root 运行用户
RUN useradd --system --no-create-home --shell /usr/sbin/nologin hsb

WORKDIR /app

# 从 binary stage 拷入二进制
COPY --from=binary --chown=hsb:hsb /out/hsb-server /app/hsb-server

# 创建挂载点目录（实际内容由 volume mount 提供）
RUN mkdir -p /app/config /app/data /app/logs \
    && chown -R hsb:hsb /app

USER hsb

# HTTP 主端口 | HTTP 管理端口 | HL7 TCP/MLLP | gRPC
EXPOSE 8080 8081 2575 10051

# 健康检查：调用 /health 端点
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -sf http://localhost:8080/health || exit 1

ENTRYPOINT ["/app/hsb-server"]
CMD ["start", "--config", "/app/config/hsb.toml"]
