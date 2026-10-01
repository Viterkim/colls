#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

package_dir=$(mktemp -d "${TMPDIR:-/tmp}/colls-package.XXXXXX")
trap 'rm -rf -- "$package_dir"' EXIT
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml)

cargo publish -p colls --dry-run --locked --target-dir "$package_dir/target" "$@"

cd -- "$package_dir/target/package/colls-$version"
export CARGO_TARGET_DIR="$package_dir/target"
cargo +1.89.0 test --all-targets --locked
cargo +1.89.0 test --all-targets --locked --no-default-features
RUSTDOCFLAGS="-D warnings" cargo +1.89.0 doc --no-deps --locked
cargo +1.89.0 check --lib --locked --target wasm32-unknown-unknown
cargo +1.89.0 check --lib --locked --target wasm32-unknown-unknown --no-default-features
cargo +1.89.0 check --lib --locked --target thumbv6m-none-eabi --no-default-features
