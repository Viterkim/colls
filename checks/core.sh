#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

cargo fmt --all --check

cargo +1.89.0 check --workspace --all-targets
cargo +1.89.0 test --workspace
cargo +1.89.0 test --workspace --no-default-features

cargo check --workspace --all-targets
cargo test --workspace
cargo test --workspace --no-default-features

cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --no-default-features

cargo check -p colls --lib --target wasm32-unknown-unknown
cargo check -p colls --lib --target wasm32-unknown-unknown --no-default-features
cargo check -p colls --lib --target thumbv6m-none-eabi --no-default-features

cargo package -p colls --allow-dirty --locked
