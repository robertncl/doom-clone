#!/usr/bin/env bash
# Build doom for macOS.
#
# macOS doesn't ship X11, so the easiest path is to use XQuartz
# (https://www.xquartz.org/), which installs an X server plus the X11
# headers and libraries at /opt/X11. Once installed, start XQuartz
# before launching ./doom.
#
# Audio: install sox via Homebrew (`brew install sox`) and the game
# will pipe samples to `play`. Without sox/aplay/paplay audio silently
# disables.
#
# Works on both Apple Silicon and Intel; clang is the default cc on macOS.
set -e

XQUARTZ_PREFIX=/opt/X11
if [ ! -d "$XQUARTZ_PREFIX/include/X11" ]; then
    echo "Error: XQuartz headers not found at $XQUARTZ_PREFIX/include/X11"
    echo "Install XQuartz from https://www.xquartz.org/ and re-run."
    exit 1
fi

clang -O2 -Wall -Wno-unused-parameter \
      -I"$XQUARTZ_PREFIX/include" \
      -L"$XQUARTZ_PREFIX/lib" \
      -o doom doom.c -lX11 -lm
echo "Build OK: ./doom  (launch XQuartz first, then run)"
