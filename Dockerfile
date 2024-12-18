# Dockerfile for creating a statically-linked Rust application using docker's
# multi-stage build feature. This also leverages the docker build cache to avoid
# re-downloading dependencies if they have not changed.
FROM rust:latest AS build
WORKDIR /usr/src

# Download the target for static linking.
RUN rustup target add x86_64-unknown-linux-musl

# INstall dependencies
RUN apt update && \
    apt install -y libasound2-dev libudev-dev libx11-dev libxcursor-dev libxcb1-dev libxi-dev libxkbcommon-dev libxkbcommon-x11-dev

# Copy the source and build the application.
COPY . .
RUN cargo build --release -p artbabo
RUN cargo install --target x86_64-unknown-linux-musl --path ./backend

# Copy the statically-linked binary into a scratch container.
FROM scratch
COPY --from=build /usr/local/cargo/bin/artbabo .
USER 1000
CMD ["./artbabo"]FROM rust:1.35.0 AS build
WORKDIR /usr/src