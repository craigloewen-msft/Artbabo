# Dockerfile for creating a statically-linked Rust application using docker's
# multi-stage build feature. This also leverages the docker build cache to avoid
# re-downloading dependencies if they have not changed.
# FROM "mcr.microsoft.com/devcontainers/rust:1-1-bullseye" AS builder
FROM rust:1.83 AS builder
WORKDIR /usr/src/

# INstall dependencies
RUN apt update && \
    apt install -y libasound2-dev libudev-dev libx11-dev libxcursor-dev libxcb1-dev libxi-dev libxkbcommon-dev libxkbcommon-x11-dev portaudio19-dev build-essential libpulse-dev libdbus-1-dev 

RUN cargo install wasm-bindgen-cli

RUN rustup target install wasm32-unknown-unknown

COPY backend ./backend
COPY frontend ./frontend
COPY server_responses ./server_responses
COPY event_work_server ./event_work_server
COPY Cargo.toml Cargo.lock ./

RUN cargo build -p artbabo_frontend --release --target wasm32-unknown-unknown

RUN wasm-bindgen --no-typescript --target web --out-dir ./website_src/ --out-name "mygame" ./target/wasm32-unknown-unknown/release/artbabo_frontend.wasm

RUN cargo install --path ./backend

# Copy the statically-linked binary into a scratch container.
FROM debian:bookworm

RUN apt update && \
    apt install -y libasound2-dev libudev-dev libx11-dev libxcursor-dev libxcb1-dev libxi-dev libxkbcommon-dev libxkbcommon-x11-dev portaudio19-dev build-essential libpulse-dev libdbus-1-dev 

WORKDIR /usr/src/backend/
COPY --from=builder /usr/local/cargo/bin/artbabo /usr/src/backend/
COPY --from=builder /usr/src/website_src /usr/src/backend/website_src
EXPOSE 8000
CMD ["artbabo"]