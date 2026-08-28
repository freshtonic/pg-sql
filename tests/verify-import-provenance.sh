#!/usr/bin/env bash

set -euo pipefail

readonly repo_root="$(git rev-parse --show-toplevel)"
readonly verifier="$repo_root/scripts/verify-import-provenance"

"$verifier"

test_dir="$(mktemp -d)"
trap 'rm -rf "$test_dir"' EXIT

git clone --quiet --no-local "$repo_root" "$test_dir/repository"
mkdir -p "$test_dir/repository/scripts"
cp "$verifier" "$test_dir/repository/scripts/verify-import-provenance"
cp "$repo_root/docs/import-provenance.tsv" "$test_dir/repository/docs/import-provenance.tsv"

(
  cd "$test_dir/repository"
  scripts/verify-import-provenance

  wrong_gitlink="$(git rev-parse HEAD)"
  git update-index --cacheinfo "160000,$wrong_gitlink,vendor/postgres"

  if scripts/verify-import-provenance >/dev/null 2>&1; then
    echo "expected verification to reject a changed PostgreSQL gitlink" >&2
    exit 1
  fi
)
