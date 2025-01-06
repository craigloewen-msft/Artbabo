# Artbabo

Based on Jackbox's bidiots (Thanks Jackbox for the great games!) it's a rust based web based game.

Site is available at: https://artbabo-bub2g5b5e3awg3gp.eastus-01.azurewebsites.net

## Debugging

- Run `cd backend && cargo watch -cx run` to debug backend
- Run `cd frontend && cargo watch -cx "run --target wasm32-unknown-unknown"` to debug frontend

## Deploying

`docker build -t craigsdevcontainers.azurecr.io/artbabo_frontend:latest .`
`docker push craigsdevcontainers.azurecr.io/artbabo_frontend:latest`
