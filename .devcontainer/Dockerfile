FROM "mcr.microsoft.com/devcontainers/rust:1-1-bullseye"

# Install Node

RUN apt-get update && apt-get install -y ca-certificates curl gnupg
RUN mkdir -p /etc/apt/keyrings
RUN curl -fsSL https://deb.nodesource.com/gpgkey/nodesource-repo.gpg.key | gpg --dearmor -o /etc/apt/keyrings/nodesource.gpg

ENV NODE_MAJOR=20
RUN curl -fsSL https://deb.nodesource.com/setup_$NODE_MAJOR.x | bash -
RUN apt-get update && apt-get install nodejs -y
RUN echo "Node: " && node -v
RUN echo "NPM: " && npm -v