#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

fail() {
  echo "workspace verification failed: $*" >&2
  exit 1
}

test -f Cargo.lock || fail "Cargo.lock is not committed"
test -f .recursa-revision || fail ".recursa-revision is missing"

revision="$(tr -d '[:space:]' < .recursa-revision)"
case "$revision" in
  *[!0-9a-f]*|'') fail ".recursa-revision must contain one lowercase Git SHA" ;;
esac
test "${#revision}" -eq 40 || fail ".recursa-revision must contain a full 40-character Git SHA"

command -v jq >/dev/null || fail "jq is required"
metadata="$(cargo metadata --locked --no-deps --format-version 1)"
recursa_root="$(cd "$repo_root/../recursa" && pwd)"

jq -e --arg root "$repo_root" --arg recursa "$recursa_root" '
  ([.packages[] | select(.manifest_path == ($root + "/Cargo.toml"))] | length == 1) and
  ([.packages[] | select(.manifest_path == ($root + "/Cargo.toml"))][0] as $package |
    $package.name == "pg-sql" and
    $package.version == "0.1.0" and
    $package.edition == "2024" and
    $package.rust_version == "1.88" and
    $package.license == "MIT" and
    $package.publish == [] and
    any($package.dependencies[]; .name == "recursa" and .kind == null and .path == ($recursa + "/recursa")) and
    any($package.dependencies[]; .name == "recursa-codegen" and .kind == "build" and .path == ($recursa + "/recursa-codegen"))) and
  ([.packages[] | select(.manifest_path == ($root + "/pg-oracle/Cargo.toml") or .manifest_path == ($root + "/migration-tool/Cargo.toml"))] |
    length == 2 and all(.[]; .publish == []))
' >/dev/null <<<"$metadata" || fail "Cargo workspace metadata does not match the repository contract"

rg -q '^resolver = "3"$' Cargo.toml || fail "workspace resolver must be 3"
rg -q '^members = \["pg-oracle", "migration-tool"\]$' Cargo.toml || fail "workspace members are incorrect"

test -x scripts/verify-recursa-revision || fail "scripts/verify-recursa-revision is missing or not executable"
test -x pg-oracle/scripts/build-pg.sh || fail "pg-oracle/scripts/build-pg.sh is not executable"

if rg -q 'cd "\$PG_SRC"|make -C "\$PG_SRC"' pg-oracle/scripts/build-pg.sh; then
  fail "PostgreSQL must be configured and built outside the source submodule"
fi
rg -q '^unset PROFILE$' pg-oracle/scripts/build-pg.sh || fail "PostgreSQL build does not isolate Cargo's PROFILE variable"

for workflow in .github/workflows/ci.yml .github/workflows/recursa-main.yml; do
  test -f "$workflow" || fail "$workflow is missing"
  rg -q 'working-directory: pg-sql' "$workflow" || fail "$workflow does not run from the pg-sql checkout"
  rg -q 'path: pg-sql' "$workflow" || fail "$workflow does not check pg-sql out into its own directory"
  rg -q 'path: recursa' "$workflow" || fail "$workflow does not create a sibling Recursa checkout"
done

echo "workspace contract verified"
