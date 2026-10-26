#!/usr/bin/env bash
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

if [[ $# != 1 || ! $1 =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
    printf 'Usage: %s 0.1.0\n' "$0" >&2
    exit 1
fi

version=$1
sed -i -E "s/^version = \"[^\"]+\"$/version = \"$version\"/" Cargo.toml
shopt -s globstar nullglob
for doc in README.md colls/docs/**/*.md; do
    sed -i -E \
        -e "s/^(colls = \")[^\"]+/\\1$version/" \
        -e "s/^(colls = \{ version = \")[^\"]+/\\1$version/" \
        "$doc"
done

# Keep the dependency versions, only refresh our package metadata.
cargo metadata --offline --format-version 1 > /dev/null

printf 'colls is now %s\n' "$version"
