# Artbabo

Based on Jackbox's bidiots (Thanks Jackbox for the great games!) it's a rust based web based game.

Site is available at: http://artbabo-bub2g5b5e3awg3gp.eastus-01.azurewebsites.net/

## Debugging

- Run `cargo watch -cx "run -p artbabo"` to debug backend
- Run `cd frontend && cargo watch -cx "run --target wasm32-unknown-unknown"` to debug frontend

## Releasing

- Run this code to build the website

```
cd frontend
cargo build --release --target wasm32-unknown-unknown
asm-bindgen --no-typescript --target web --out-dir ../docs/ --out-name "mygame" ../target/wasm32-unknown-unkno
wn/release/artbabo_frontend.wasm
```

## Deploying

### Backend

`docker build -t craigsdevcontainers.azurecr.io/artbabo:latest .`
`docker push craigsdevcontainers.azurecr.io/artbabo:latest`

### Frontend

`cd docs`
`docker build -t craigsdevcontainers.azurecr.io/artbabo_frontend:latest .`
`docker push craigsdevcontainers.azurecr.io/artbabo_frontend:latest`