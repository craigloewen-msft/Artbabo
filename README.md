# Artbabo

Based on Jackbox's bidiots (Thanks Jackbox for the great games!) it's a rust based web based game.

## Debugging

- Run `cargo watch -cx "run -p artbabo"` to debug backend
- Run `cd frontend && cargo watch -cx "run --target wasm32-unknown-unknown"` to debug frontend

## Releasing

- Run this code to build the website

```
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --no-typescript --target web \
    --out-dir ./out/ \
    --out-name "mygame" \
    ./target/wasm32-unknown-unknown/release/mygame.wasm
```

Move the files to /docs
