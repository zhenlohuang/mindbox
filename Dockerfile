# syntax=docker/dockerfile:1.7

FROM rust:1.93-bullseye AS builder
WORKDIR /app

COPY Cargo.toml ./
COPY mindbox-common ./mindbox-common
COPY mindbox-kernel ./mindbox-kernel
COPY mindbox-server ./mindbox-server
COPY mindbox-cli ./mindbox-cli

RUN cargo build --release -p mindbox-server

FROM nvidia/cuda:12.8.1-cudnn-runtime-ubuntu24.04

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    build-essential \
    cmake \
    curl \
    git \
    python3 \
    python3-pip \
    python3-venv \
    supervisor \
    nodejs \
    npm \
    unzip \
    libglib2.0-0 \
    libsm6 \
    libxext6 \
    libxrender1 \
    libxcb1 \
    && rm -rf /var/lib/apt/lists/*

RUN pip3 install --no-cache-dir --break-system-packages \
    uv \
    PyYAML \
    tensorboard
RUN npm install -g @anthropic-ai/claude-code

WORKDIR /mindbox

COPY --from=builder /app/target/release/mindbox-server /mindbox/bin/mindbox-server
COPY docker/docker-entrypoint.sh /mindbox/bin/docker-entrypoint.sh
COPY docker/supervisord.conf /etc/supervisor/conf.d/mindbox.conf
RUN chmod +x /mindbox/bin/docker-entrypoint.sh \
    && groupadd --system mindbox \
    && useradd --system --gid mindbox --create-home --home-dir /home/mindbox --shell /usr/sbin/nologin mindbox \
    && mkdir -p /home/mindbox/.config /home/mindbox/.local/bin \
    && chown -R mindbox:mindbox /mindbox /home/mindbox

ENV MINDBOX_KERNEL=claude-code
ENV MINDBOX_DATA_ROOT=/mindbox
ENV MINDBOX_PORT=8080

EXPOSE 8080
EXPOSE 6006

USER mindbox
ENV PATH="/home/mindbox/.local/bin:${PATH}"

ENTRYPOINT ["/mindbox/bin/docker-entrypoint.sh"]
