#!/usr/bin/env bash
# Builds the PostgreSQL checkout far enough that pg-oracle can link its
# parser. Idempotent: safe to re-run.
#
# PostgreSQL lives in the `pg-sql/vendor/postgres` Git submodule,
# pinned to the REL_17_9 tag.
set -euo pipefail

# PostgreSQL's makefiles interpret PROFILE as compiler profiling flags. Cargo
# exports PROFILE=debug/release to build scripts, so do not leak it into make.
unset PROFILE

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PG_SRC="$REPO_ROOT/vendor/postgres"
PG_BUILD="${1:-${PG_ORACLE_PG_BUILD_DIR:-$REPO_ROOT/target/postgres-17.9}}"

if [ ! -f "$PG_SRC/configure" ]; then
  echo "PostgreSQL source not found at $PG_SRC" >&2
  echo "Initialize the submodule: git submodule update --init vendor/postgres" >&2
  exit 1
fi

tag="$(git -C "$PG_SRC" describe --tags 2>/dev/null || echo unknown)"
echo "PostgreSQL source: $PG_SRC ($tag)"
case "$tag" in
  REL_17_*) ;;
  *) echo "WARNING: expected a REL_17_* tag, got '$tag'" >&2 ;;
esac

mkdir -p "$PG_BUILD"

if [ ! -f "$PG_BUILD/src/Makefile.global" ]; then
  echo "Configuring..."
  ( cd "$PG_BUILD" && "$PG_SRC/configure" --without-icu --without-zlib --without-readline )
fi

echo "Building (this is slow the first time)..."
make -C "$PG_BUILD" -j"$(getconf _NPROCESSORS_ONLN)" all

echo "Done. Generated sources and static libs are in $PG_BUILD."
