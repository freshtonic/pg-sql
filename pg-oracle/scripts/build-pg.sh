#!/usr/bin/env bash
# Builds the PostgreSQL checkout far enough that pg-oracle can link its
# parser. Idempotent: safe to re-run.
#
# PostgreSQL lives in the `pg-sql/vendor/postgres` Git submodule,
# pinned to the REL_17_9 tag.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PG_SRC="$SCRIPT_DIR/../../pg-sql/vendor/postgres"

if [ ! -f "$PG_SRC/configure" ]; then
  echo "PostgreSQL source not found at $PG_SRC" >&2
  echo "Initialize the submodule: git submodule update --init pg-oracle/vendor/postgres" >&2
  exit 1
fi

tag="$(git -C "$PG_SRC" describe --tags 2>/dev/null || echo unknown)"
echo "PostgreSQL source: $PG_SRC ($tag)"
case "$tag" in
  REL_17_*) ;;
  *) echo "WARNING: expected a REL_17_* tag, got '$tag'" >&2 ;;
esac

if [ ! -f "$PG_SRC/src/Makefile.global" ]; then
  echo "Configuring..."
  ( cd "$PG_SRC" && ./configure --without-icu --without-zlib --without-readline )
fi

echo "Building (this is slow the first time)..."
make -C "$PG_SRC" -j"$(getconf _NPROCESSORS_ONLN)" all

echo "Done. Generated sources and static libs are in place."
