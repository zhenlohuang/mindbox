# syntax=docker/dockerfile:1.7

FROM rust:1.93-bullseye AS builder
WORKDIR /app

COPY Cargo.toml ./
COPY mindbox-common ./mindbox-common
COPY mindbox-kernel ./mindbox-kernel
COPY mindbox-server ./mindbox-server
COPY mindbox-cli ./mindbox-cli

RUN cargo build --release -p mindbox-server -p mindbox-cli

FROM nvidia/cuda:12.8.1-cudnn-runtime-ubuntu24.04

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    git \
    python3 \
    python3-pip \
    supervisor \
    nodejs \
    npm \
    && rm -rf /var/lib/apt/lists/*

RUN pip3 install --no-cache-dir --break-system-packages tensorboard
RUN npm install -g @anthropic-ai/claude-code

WORKDIR /workspace

COPY --from=builder /app/target/release/mindbox-server /mindbox/bin/mindbox-server
COPY --from=builder /app/target/release/mindbox-cli /mindbox/bin/mindbox-cli
COPY docker/docker-entrypoint.sh /mindbox/bin/docker-entrypoint.sh
COPY docker/supervisord.conf /etc/supervisor/conf.d/mindbox.conf
RUN chmod +x /mindbox/bin/docker-entrypoint.sh

ENV MINDBOX_KERNEL=claude-code
ENV MINDBOX_DATA_ROOT=/mindbox
ENV MINDBOX_PORT=8080

EXPOSE 8080
EXPOSE 6006

ENTRYPOINT ["/mindbox/bin/docker-entrypoint.sh"]
