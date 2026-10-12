# 第一阶段：构建前端主题
FROM --platform=$BUILDPLATFORM node:20-alpine AS frontend-builder

ARG FRONTEND_2024_REF=main
ARG FRONTEND_2023_REF=main

RUN apk add --no-cache git python3 make g++
RUN corepack enable && corepack prepare pnpm@9.15.9 --activate

WORKDIR /build

RUN git clone --filter=blob:none --no-checkout https://github.com/vastsa/FileCodeBoxFronted.git /build/fronted-2024 && \
    cd /build/fronted-2024 && \
    git fetch --depth 1 origin "${FRONTEND_2024_REF}" && \
    git checkout --detach FETCH_HEAD && \
    pnpm install --frozen-lockfile --prod=false && \
    VITE_GIT_COMMIT="$(git rev-parse HEAD)" pnpm run build

RUN git clone --filter=blob:none --no-checkout https://github.com/vastsa/FileCodeBoxFronted2023.git /build/fronted-2023 && \
    cd /build/fronted-2023 && \
    git fetch --depth 1 origin "${FRONTEND_2023_REF}" && \
    git checkout --detach FETCH_HEAD && \
    npm install --legacy-peer-deps && \
    npm run build

# 第二阶段：构建 Rust 二进制
FROM rust:1.85-bookworm AS rust-builder
WORKDIR /app
COPY . .
RUN cargo build --release -p fcb-server

# 第三阶段：极简运行环境 (仅 libc + ca-certificates + sqlite)
FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tzdata sqlite3 \
    && rm -rf /var/lib/apt/lists/*

RUN ln -sf /usr/share/zoneinfo/Asia/Shanghai /etc/localtime && echo 'Asia/Shanghai' > /etc/timezone

COPY --from=frontend-builder /build/FileCodeBox/themes/2024 ./themes/2024
COPY --from=frontend-builder /build/fronted-2023/dist ./themes/2023
COPY --from=rust-builder /app/target/release/fcb-server /usr/local/bin/fcb-server

ENV HOST="0.0.0.0" \
    PORT=12345 \
    APP_ENV="production"

EXPOSE 12345

CMD ["fcb-server"]
