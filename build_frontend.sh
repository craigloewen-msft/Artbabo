cd frontend
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --no-typescript --target web --out-dir ../docs/ --out-name "mygame" ../target/wasm32-unknown-unknown/release/artbabo_frontend.wasm
cd ../docs
docker build -t craigsdevcontainers.azurecr.io/artbabo_frontend:latest .