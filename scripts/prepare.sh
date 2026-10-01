#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

if [[ -n "$(git status --porcelain)" ]]; then
    printf 'Commit first man\n' >&2
    exit 1
fi

bash check.sh

# Checks can change lockfiles, export the version that was actually checked.
if [[ -n "$(git status --porcelain)" ]]; then
    printf 'Checks changed files. Review and commit them first man\n' >&2
    exit 1
fi

release_dir=$(mktemp -d "${TMPDIR:-/tmp}/colls-release.XXXXXX")
git archive HEAD | tar -xf - -C "$release_dir"
release_url="https://github.com/Viterkim/colls/blob/$(git rev-parse HEAD)"

# Only the published copy gets full links.
sed -i -E "s@\]\((\./)?colls/@](${release_url}/colls/@g" "$release_dir/README.md"

cd -- "$release_dir"
bash checks/package.sh

printf '\nDry run bingo! You are in %s\n' "$release_dir"
printf 'cargo publish -p colls\n'
printf 'exit this shell when you are done.\n\n'
exec "${SHELL:-bash}" -i
