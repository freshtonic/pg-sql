#!/usr/bin/env bash
# Builds the pinned PostgreSQL release of one target version far enough that
# pg-oracle can link its parser. Idempotent: safe to re-run.
#
# Usage: build-pg.sh [FEATURE [TREE]]
#
#   FEATURE  a version feature from pg-oracle/pins.tsv (default: pg17).
#   TREE     the directory for the source tree and the build
#            (default: target/pg-trees/<ref>, for example
#            target/pg-trees/REL_16_15).
#
# The pinned commit comes from pg-oracle/pins.tsv. The script extracts that
# commit from the object store of the `vendor/postgres` Git submodule with
# `git archive`, then configures and builds it in TREE. The submodule
# checkout does not change. The file TREE/pg-oracle-source-commit records the
# commit of the tree. If the pin moves, the script removes the tree and
# builds it again, because PostgreSQL is configured without dependency
# tracking.
set -euo pipefail

# PostgreSQL's makefiles interpret PROFILE as compiler profiling flags. Cargo
# exports PROFILE=debug/release to build scripts, so do not leak it into make.
unset PROFILE

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PG_REPO="$REPO_ROOT/vendor/postgres"
PINS="$SCRIPT_DIR/../pins.tsv"

feature="${1:-pg17}"
pin="$(awk -F'\t' -v feature="$feature" '$1 == feature { print $2 "\t" $3 }' "$PINS")"
if [ -z "$pin" ]; then
  echo "No pin for the version feature '$feature' in $PINS" >&2
  exit 1
fi
ref="${pin%%$'\t'*}"
commit="${pin#*$'\t'}"
PG_TREE="${2:-$REPO_ROOT/target/pg-trees/$ref}"
STAMP="$PG_TREE/pg-oracle-source-commit"

if [ ! -e "$PG_REPO/.git" ]; then
  echo "PostgreSQL repository not found at $PG_REPO" >&2
  echo "Initialize the submodule: git submodule update --init vendor/postgres" >&2
  exit 1
fi

echo "PostgreSQL $ref ($commit) for $feature, tree $PG_TREE"

if [ -e "$PG_TREE" ] && [ "$(cat "$STAMP" 2>/dev/null || true)" != "$commit" ]; then
  echo "The tree is not from $commit. Removing it."
  rm -rf "$PG_TREE"
fi

if [ ! -e "$STAMP" ]; then
  if ! git -C "$PG_REPO" cat-file -e "$commit^{commit}" 2>/dev/null; then
    echo "The commit $commit ($ref) is not in $PG_REPO." >&2
    echo "Fetch it: git -C vendor/postgres fetch origin $commit" >&2
    exit 1
  fi
  echo "Extracting $ref..."
  partial="$PG_TREE.partial"
  rm -rf "$partial"
  mkdir -p "$partial"
  git -C "$PG_REPO" archive --format=tar "$commit" | tar -x -C "$partial"
  echo "$commit" > "$partial/pg-oracle-source-commit"
  mv "$partial" "$PG_TREE"
fi

if [ ! -f "$PG_TREE/src/Makefile.global" ]; then
  echo "Configuring..."
  ( cd "$PG_TREE" && ./configure --without-icu --without-zlib --without-readline )
fi

echo "Building (this is slow the first time)..."
make -C "$PG_TREE" -j"$(getconf _NPROCESSORS_ONLN)" all

echo "Done. Generated sources and static libs are in $PG_TREE."
