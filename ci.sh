#!/usr/bin/env bash
# ci.sh — the gate for this repo (bupha shell runner; no GitHub Actions).
#
#   ci.sh                       full gate: fmt → clippy → test → doc
#   ci.sh fmt|clippy|test|doc   one stage
#
# Deps resolve from git (the manifest carries no [patch] tables), so this runs
# in a bare clone or in a bupha worktree of the pushed sha with no sibling
# checkouts. Private handl-lang deps need auth: export HANDL_TOKEN /
# BUPHA_TOKEN, or rely on the gh credential helper / ssh config. Local
# sibling builds are an opt-in dev convenience via the workspace-root
# .cargo/config.toml.
set -uo pipefail
cd "$(dirname "$0")"

# Private git deps: insteadOf rewrite scoped to this process tree via
# GIT_CONFIG_* env — never touches ~/.gitconfig. A token wins when the
# watcher exports one; otherwise rewrite ssh URLs to https so the gh
# credential helper / ssh config can authenticate.
tok="${HANDL_TOKEN:-${BUPHA_TOKEN:-}}"
if [ -n "$tok" ]; then
  export GIT_CONFIG_COUNT=1
  export GIT_CONFIG_KEY_0="url.https://x-access-token:${tok}@github.com/.insteadOf"
  export GIT_CONFIG_VALUE_0="https://github.com/"
elif [ -z "${GIT_CONFIG_COUNT:-}" ]; then
  export GIT_CONFIG_COUNT=1
  export GIT_CONFIG_KEY_0="url.git@github.com:handl-lang/.insteadOf"
  export GIT_CONFIG_VALUE_0="https://github.com/handl-lang/"
fi
# Cargo must shell out to git so the insteadOf rewrite and credential
# helpers apply to git deps.
export CARGO_NET_GIT_FETCH_WITH_CLI=true

stage="${1:-all}"
fail=0
note() { printf '\n== %s\n' "$*"; }
failed() { echo "   FAILED: $*" >&2; fail=1; }

run_fmt()    { note fmt;    cargo fmt --check || failed 'cargo fmt --check'; }
run_clippy() { note clippy; cargo clippy --all-targets -- -D warnings || failed 'cargo clippy'; }
run_test()   { note test;   cargo test || failed 'cargo test'; }
run_doc()    { note doc;    RUSTDOCFLAGS='-D warnings' cargo doc --no-deps || failed 'cargo doc'; }

case "$stage" in
    all) run_fmt; run_clippy; run_test; run_doc ;;
    fmt | clippy | test | doc) "run_$stage" ;;
    *) echo 'usage: ci.sh [all|fmt|clippy|test|doc]' >&2; exit 2 ;;
esac

if [ "$fail" -ne 0 ]; then echo -e '\nci: FAILED' >&2; exit 1; fi
echo -e '\nci: ok'
