# Dockerfile for creating a statically-linked Rust application using docker's
# multi-stage build feature. This also leverages the docker build cache to avoid
# re-downloading dependencies if they have not changed.
FROM "mcr.microsoft.com/devcontainers/rust:1-1-bullseye" AS builder
# FROM rust:1.83 
WORKDIR /usr/src/

# INstall dependencies
RUN apt update && \
    apt install -y libasound2-dev libudev-dev libx11-dev libxcursor-dev libxcb1-dev libxi-dev libxkbcommon-dev libxkbcommon-x11-dev portaudio19-dev build-essential libpulse-dev libdbus-1-dev 

RUN USER=root cargo new myapp
WORKDIR /usr/src/myapp
RUN USER=root cargo new backend
RUN USER=root cargo new frontend
RUN USER=root cargo new server_responses
COPY Cargo.toml Cargo.lock /usr/src/myapp/
COPY backend/Cargo.toml backend/Cargo.lock /usr/src/myapp/backend
COPY frontend/Cargo.toml /usr/src/myapp/frontend
COPY server_responses/Cargo.toml /usr/src/myapp/server_responses

RUN cargo build --release -p artbabo

# Copy the source and build the application.
COPY server_responses/src /usr/src/myapp/server_responses/src
COPY backend/src /usr/src/myapp/frontend/src

RUN cargo build --release -p artbabo
RUN cargo install --path ./backend

# Copy the statically-linked binary into a scratch container.
FROM debian:bullseye

RUN apt update && \
    apt install -y libasound2-dev libudev-dev libx11-dev libxcursor-dev libxcb1-dev libxi-dev libxkbcommon-dev libxkbcommon-x11-dev portaudio19-dev build-essential libpulse-dev libdbus-1-dev 

COPY --from=builder /usr/local/cargo/bin/artbabo /usr/local/bin/artbabo
EXPOSE 8081