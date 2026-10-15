#!/usr/bin/env bash
# Builds the pinned PostgreSQL release of one target version far enough that
# pg-oracle can link its parser. Idempotent: safe to re-run.
#
# Usage: build-pg.sh [FEATURE [TREE]]
#
#   FEATURE  a version feature from pg-oracle/pins.tsv (default: pg17).
#   TREE     the directory for the source tree and the build. The default
#            is the shared cache (ADR 0012): <cache>/<ref>-<commit>, where
#            <cache> is $PG_SQL_POSTGRES_CACHE, else
#            ${XDG_CACHE_HOME:-~/.cache}/pg-sql/postgres. Both oracle build
#            scripts use this default, so every checkout and worktree on the
#            machine builds each pinned release once.
#
# The pinned commit comes from pg-oracle/pins.tsv. The script extracts that
# commit from the object store of the `vendor/postgres` Git submodule with
# `git archive`, then configures and builds it in TREE. The submodule
# checkout does not change. The file TREE/pg-oracle-source-commit records the
# commit of the tree. The default tree name carries the commit, so a moved
# pin gets a new tree beside the old one; a TREE given by hand that holds
# another commit is removed and built again, because PostgreSQL is configured
# without dependency tracking. A lock file beside the tree serialises builds
# of the same tree from several checkouts.
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
cache="${PG_SQL_POSTGRES_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/pg-sql/postgres}"
PG_TREE="${2:-$cache/$ref-$commit}"
STAMP="$PG_TREE/pg-oracle-source-commit"

if [ ! -e "$PG_REPO/.git" ]; then
  echo "PostgreSQL repository not found at $PG_REPO" >&2
  echo "Initialize the submodule: git submodule update --init vendor/postgres" >&2
  exit 1
fi

echo "PostgreSQL $ref ($commit) for $feature, tree $PG_TREE"

# One build of a tree at a time, across checkouts. The lock is held by file
# descriptor 9 for the rest of the script; perl takes it because macOS has no
# flock(1).
mkdir -p "$(dirname "$PG_TREE")"
exec 9>"$PG_TREE.lock"
if ! perl -e 'use Fcntl ":flock"; open(my $fh, ">&=", 9) or die; exit(flock($fh, LOCK_EX | LOCK_NB) ? 0 : 1)'; then
  echo "Another build of $PG_TREE is running; waiting for it."
  perl -e 'use Fcntl ":flock"; open(my $fh, ">&=", 9) or die; flock($fh, LOCK_EX) or die'
fi

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

# PostgreSQL links its generated headers into src/include with absolute
# paths and guards the links with `header-stamp` files, so a tree that was
# moved (into the cache, or by PG_SQL_POSTGRES_CACHE) holds dangling links
# that make reports as up to date. Remove the dangling links and, always, the
# stamps: make then links the headers again, which costs a few `ln -s`.
find "$PG_TREE" -type l ! -exec test -e {} \; -delete
find "$PG_TREE/src/include" -name header-stamp -delete

echo "Building (this is slow the first time)..."
make -C "$PG_TREE" -j"$(getconf _NPROCESSORS_ONLN)" all

echo "Done. Generated sources and static libs are in $PG_TREE."
