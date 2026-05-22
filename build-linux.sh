#!/usr/bin/env bash
# Build doom for Linux / ARM64 Linux (X11 backend).
# Verified working on aarch64 Ubuntu 24.04 with WSLg.
set -e
gcc -O2 -Wall -Wno-unused-parameter -o doom doom.c -lX11 -lm
echo "Build OK: ./doom  (run with no args for windowed, --headless for tests)"
