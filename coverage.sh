#!/usr/bin/env bash
# Line/region coverage for the game code.
#
# `src/tests/` is excluded from the report on purpose: test bodies run by
# definition, so counting them scores ~100% on thousands of lines and flatters
# the total. What's left is coverage of the game itself.
#
# Takes a while (~15 min): the play-through tests render tens of thousands of
# frames, and instrumented rendering is slow. `cargo test --release` is the
# everyday loop at well under a minute.
#
#   ./coverage.sh              # summary table
#   ./coverage.sh --html       # full HTML report in target/llvm-cov/html
#   ./coverage.sh --open       # ...and open it
#
# Needs: cargo install cargo-llvm-cov && rustup component add llvm-tools-preview
set -euo pipefail

MODE=(--summary-only)
case "${1:-}" in
  --html) MODE=(--html) ;;
  --open) MODE=(--html --open) ;;
  "") ;;
  *) echo "usage: $0 [--html|--open]" >&2; exit 2 ;;
esac

# --release keeps the play-through tests (which run thousands of rendered
# frames) from dominating the wall time.
exec cargo llvm-cov --release --ignore-filename-regex 'src/tests/' "${MODE[@]}"
