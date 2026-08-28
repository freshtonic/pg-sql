#!/usr/bin/env bash

set -euo pipefail

readonly repo_root="$(git rev-parse --show-toplevel)"
readonly verifier="$repo_root/scripts/verify-import-provenance"

"$verifier"

test_dir="$(mktemp -d)"
trap 'rm -rf "$test_dir"' EXIT

git clone --quiet --no-local "$repo_root" "$test_dir/repository"
git -C "$test_dir/repository" rm --quiet tests/verify-import-provenance.sh
mkdir -p "$test_dir/repository/scripts"
cp "$verifier" "$test_dir/repository/scripts/verify-import-provenance"
cp "$repo_root/docs/import-provenance.tsv" "$test_dir/repository/docs/import-provenance.tsv"
cp "$repo_root/docs/import-provenance.legacy-commit" "$test_dir/repository/docs/import-provenance.legacy-commit"
cp "$repo_root/docs/import-provenance.legacy-tree" "$test_dir/repository/docs/import-provenance.legacy-tree"

(
  cd "$test_dir/repository"
  scripts/verify-import-provenance

  wrong_gitlink="$(git rev-parse HEAD)"
  git update-index --cacheinfo "160000,$wrong_gitlink,vendor/postgres"

  if scripts/verify-import-provenance >/dev/null 2>&1; then
    echo "expected verification to reject a changed PostgreSQL gitlink" >&2
    exit 1
  fi

  git reset --quiet
  printf '\n# changed\n' >> Cargo.toml

  if scripts/verify-import-provenance >/dev/null 2>&1; then
    echo "expected verification to reject a changed imported file" >&2
    exit 1
  fi

  git checkout --quiet -- Cargo.toml
  printf '// untracked contamination\n' > src/untracked-immutable-input.rs

  if scripts/verify-import-provenance >/dev/null 2>&1; then
    echo "expected verification to reject an untracked imported-source file" >&2
    exit 1
  fi

  git add src/untracked-immutable-input.rs

  if scripts/verify-import-provenance >/dev/null 2>&1; then
    echo "expected verification to reject a staged imported-source file" >&2
    exit 1
  fi
)
