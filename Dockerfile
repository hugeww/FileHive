# 第一阶段：构建前端主题
FROM --platform=$BUILDPLATFORM node:20-alpine AS frontend-builder

WORKDIR /build/frontend
RUN corepack enable && corepack prepare pnpm@9.15.9 --activate

COPY frontend/package.json frontend/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile --prod=false
COPY frontend ./
RUN pnpm run build-only --outDir /build/themes/2024

# 第二阶段：构建 Rust 二进制
FROM rust:1.85-bookworm AS rust-builder
WORKDIR /app
COPY . .
RUN cargo build --release -p filehive-server

# 第三阶段：极简运行环境 (仅 libc + ca-certificates + sqlite)
FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tzdata sqlite3 libssl3 \
    && rm -rf /var/lib/apt/lists/*

RUN ln -sf /usr/share/zoneinfo/Asia/Shanghai /etc/localtime && echo 'Asia/Shanghai' > /etc/timezone

COPY --from=frontend-builder /build/themes/2024 ./themes/2024
COPY --from=rust-builder /app/target/release/filehive /usr/local/bin/filehive

ENV HOST="0.0.0.0" \
    PORT=12345 \
    APP_ENV="production"

EXPOSE 12345

CMD ["filehive"]
