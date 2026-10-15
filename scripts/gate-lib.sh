#!/usr/bin/env bash
# Shared by scripts/merge-gate and scripts/release-gate (ADR 0011). Source it.
#
# The merge gate configuration: the one build of the generated grammar that
# the merge gate proves, and that `cargo loop` (.cargo/config.toml) tests in
# the edit loop. Every package of the workspace that the gates test is in one
# cargo invocation, so Cargo unifies the features and pg-sql is compiled once.
# Every feature is spelled out under `--no-default-features`: a package named
# with `-p` would otherwise carry its `default` feature, which pg-sql's own
# dependency on it does not, and the two feature sets would be two builds.
gate_packages=(-p pg-sql -p pg-psql -p pg-oracle -p pg-psql-oracle --no-default-features)
gate_features() {
  local v="$1"
  echo "pg-sql/$v,pg-sql/spans,pg-sql/arbitrary,pg-sql/postgres-oracle,pg-psql/$v,pg-psql/psql-oracle,pg-oracle/$v,pg-psql-oracle/$v"
}
merge_features="$(gate_features pg17)"

# Where the gates keep their lock. $PG_SQL_GATE_LOCK moves it.
gate_lock_file() {
  echo "${PG_SQL_GATE_LOCK:-${XDG_CACHE_HOME:-$HOME/.cache}/pg-sql/gate.lock}"
}

run() {
  echo "+ $*" >&2
  "$@"
}

# Move to the repository, drop a shell toolchain override, and check that the
# sibling recursa is at the pinned revision, so a mismatch is reported here
# and not as a codegen error minutes later.
gate_setup() {
  cd "$(git rev-parse --show-toplevel)"
  # A RUSTUP_TOOLCHAIN from the shell overrides rust-toolchain.toml.
  unset RUSTUP_TOOLCHAIN
  command -v jq >/dev/null || { echo "jq is required" >&2; exit 2; }
  scripts/verify-recursa-revision >&2
}

# One gate at a time on the machine. Two gates on ten cores finish later than
# the same two one after the other, and three swap. The lock is held by file
# descriptor 8 until the script exits; perl takes it because macOS has no
# flock(1). PG_SQL_GATE_NO_LOCK=1 skips it.
gate_lock() {
  if [ -n "${PG_SQL_GATE_NO_LOCK:-}" ]; then
    return
  fi
  local lock
  lock="$(gate_lock_file)"
  mkdir -p "$(dirname "$lock")"
  exec 8>"$lock"
  if ! perl -e 'use Fcntl ":flock"; open(my $fh, ">&=", 8) or die; exit(flock($fh, LOCK_EX | LOCK_NB) ? 0 : 1)'; then
    echo "another gate holds $lock; waiting for it (PG_SQL_GATE_NO_LOCK=1 skips the lock)" >&2
    perl -e 'use Fcntl ":flock"; open(my $fh, ">&=", 8) or die; flock($fh, LOCK_EX) or die'
    echo "gate lock acquired" >&2
  fi
}

# Remove the build artefacts of the workspace's own packages that the merge
# gate configuration does not use. Every other configuration (another target
# version, another feature set, an older recursa or grammar) is a stale
# 1 GB that nothing reads again; a full release gate left 200 GB behind
# before this. The live set comes from Cargo itself: `--message-format=json`
# names the artefact and OUT_DIR of every unit, fresh or not, so nothing the
# configuration uses is ever removed, and the prune after a build costs one
# cargo call that compiles nothing. Incremental caches are kept one per
# crate, the newest.
gate_prune() {
  local target live removed
  target="$(cargo metadata --locked --no-deps --format-version 1 | jq -r .target_directory)"
  # The merge gate configuration exactly as scripts/merge-gate builds it: the
  # release gate's CARGO_INCREMENTAL=0 must not leak in, or this call would
  # compile the configuration again under another fingerprint.
  live="$(
    env -u CARGO_INCREMENTAL cargo test --locked "${gate_packages[@]}" --features "$merge_features" --no-run \
      --message-format=json-render-diagnostics 2>/dev/null |
      jq -r '(.filenames[]?, .out_dir?) // empty' |
      grep -o "$target/[^/]*/build/[^/]*/[0-9a-f]*" | sort -u
  )"
  if [ -z "$live" ]; then
    echo "prune: cargo named no artefacts; nothing removed" >&2
    return
  fi
  removed=0
  for package in pg-sql pg-psql pg-oracle pg-psql-oracle; do
    for dir in "$target"/*/build/"$package"/*/; do
      dir="${dir%/}"
      [ -d "$dir" ] || continue
      if ! grep -qx "$dir" <<<"$live"; then
        rm -rf "$dir"
        removed=$((removed + 1))
      fi
    done
  done
  # Incremental caches: `<crate>-<hash>` per configuration; keep the newest
  # of each crate.
  for incremental in "$target"/*/incremental; do
    [ -d "$incremental" ] || continue
    for crate in $(ls "$incremental" | sed 's/-[^-]*$//' | sort -u); do
      for dir in $(ls -td "$incremental/$crate"-* 2>/dev/null | tail -n +2); do
        rm -rf "$dir"
        removed=$((removed + 1))
      done
    done
  done
  echo "prune: removed $removed stale build directories under $target" >&2
}
