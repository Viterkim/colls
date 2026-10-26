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
shopt -s globstar nullglob
link_pattern='\]\(([^)]+)\)'
repo_url='https://github.com/Viterkim/colls/blob/main/'

for doc in "$release_dir/README.md" "$release_dir"/colls/docs/**/*.md; do
    doc_copy=$(mktemp "$doc.XXXXXX")
    doc_dir=$(dirname -- "$doc")

    while IFS= read -r line || [[ -n "$line" ]]; do
        output=

        while [[ $line =~ $link_pattern ]]; do
            matched_link=${BASH_REMATCH[0]}
            target=${BASH_REMATCH[1]}
            output+=${line%%"$matched_link"*}
            line=${line#*"$matched_link"}

            case "$target" in
                "$repo_url"*) target="$release_url/${target#"$repo_url"}" ;;
                \#* | *:* | //*) ;;
                *)
                    target_path=${target%%[?#]*}
                    suffix=${target#"$target_path"}
                    source_path="$doc_dir/$target_path"

                    if [[ ! -e "$source_path" ]]; then
                        printf '%s: missing link %s\n' "${doc#"$release_dir/"}" "$target" >&2
                        exit 1
                    fi

                    target_path=$(realpath --relative-to="$release_dir" -- "$source_path")
                    target="$release_url/$target_path$suffix"
                    ;;
            esac

            output+="]($target)"
        done

        printf '%s\n' "$output$line"
    done < "$doc" > "$doc_copy"

    cat "$doc_copy" > "$doc"
    rm -- "$doc_copy"
done

cd -- "$release_dir"
bash checks/package.sh

printf '\nDry run bingo! You are in %s\n\n' "$release_dir"
printf 'Bash / Nushell:\n'
printf 'cargo publish -p colls\n'
printf 'exit this shell when you are done.\n\n'
exec "${SHELL:-bash}" -i
