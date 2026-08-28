#!/usr/bin/env bash
# Capture-only replacement for the legacy pg-oracle build helper. The parser
# and oracle sources are unchanged; only generated/build output is redirected.
set -euo pipefail

unset PROFILE

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PG_SRC="$SCRIPT_DIR/../../pg-sql/vendor/postgres"
PG_BUILD="${1:?pass an out-of-tree PostgreSQL build directory}"

if [ ! -f "$PG_SRC/configure" ]; then
  echo "PostgreSQL source not found at $PG_SRC" >&2
  exit 1
fi

tag="$(git -C "$PG_SRC" describe --tags 2>/dev/null || echo unknown)"
case "$tag" in
  REL_17_*) ;;
  *) echo "expected a PostgreSQL 17 tag, got '$tag'" >&2; exit 1 ;;
esac

mkdir -p "$PG_BUILD"
if [ ! -f "$PG_BUILD/src/Makefile.global" ]; then
  ( cd "$PG_BUILD" && "$PG_SRC/configure" --without-icu --without-zlib --without-readline )
fi

make -C "$PG_BUILD" -j"$(getconf _NPROCESSORS_ONLN)" all
