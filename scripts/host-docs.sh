#!/usr/bin/env bash
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

cargo doc -p colls --no-deps

printf '\nOpen http://127.0.0.1:8585/colls/\n'
printf 'For a remote machine: ssh -L 8585:127.0.0.1:8585 user@host\n\n'
python3 -m http.server 8585 --directory target/doc --bind 127.0.0.1
