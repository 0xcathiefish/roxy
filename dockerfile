# ---- Stage 1: Builder ----
# 使用官方的 Rust 镜像作为构建环境
FROM rust:1.89.0 AS builder

# 安装构建时需要的系统依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    liboping-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# --- 依赖缓存层 ---
# 仅复制清单文件
COPY Cargo.toml Cargo.lock ./
# 创建一个虚拟的 main.rs 来构建和缓存依赖
RUN mkdir src/ && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src/ .cargo/

# --- 编译层 ---
# 复制所有项目文件，包括 src/, .sqlx/ 等
COPY . .

# 关键：为 sqlx 启用离线模式
ENV SQLX_OFFLINE=true

# 编译真正的项目
RUN cargo build --release --locked

# ---- Stage 2: Runner ----
# 使用与 builder 相同基础发行版的 slim 镜像，保证库文件兼容
FROM debian:trixie-slim

# 安装运行时需要的动态链接库
RUN apt-get update && apt-get install -y \
    openssl \
    liboping-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# 从 builder 阶段复制编译好的二进制文件
COPY --from=builder /app/target/release/roxy /usr/local/bin/roxy

# 设置工作目录
WORKDIR /app

# 暴露端口
EXPOSE 8080

# 容器启动时执行的命令
CMD ["roxy"]
