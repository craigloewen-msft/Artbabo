# Dockerfile for creating a statically-linked Rust application using docker's
# multi-stage build feature. This also leverages the docker build cache to avoid
# re-downloading dependencies if they have not changed.
FROM rust:1.83 AS builder
# FROM rust:1.83 
WORKDIR /usr/src/myapp

# INstall dependencies
RUN apt update && \
    apt install -y libasound2-dev libudev-dev libx11-dev libxcursor-dev libxcb1-dev libxi-dev libxkbcommon-dev libxkbcommon-x11-dev portaudio19-dev build-essential libpulse-dev libdbus-1-dev 

# Copy the source and build the application.
COPY . .
RUN cargo build --release -p artbabo
RUN cargo install --path ./backend

# Copy the statically-linked binary into a scratch container.
FROM debian:bookworm

RUN apt update && \
    apt install -y libasound2-dev libudev-dev libx11-dev libxcursor-dev libxcb1-dev libxi-dev libxkbcommon-dev libxkbcommon-x11-dev portaudio19-dev build-essential libpulse-dev libdbus-1-dev 

COPY --from=builder /usr/local/cargo/bin/artbabo /usr/local/bin/artbabo
CMD ["artbabo"]