#!/bin/sh
# Build themisio.exe for Windows (x86_64-pc-windows-gnu) from Linux with MinGW-w64.
# Needs: rust-std for x86_64-pc-windows-gnu, gcc-mingw-w64-x86-64 (posix threads), cmake, nasm.
set -e
cd "$(dirname "$0")/.."
export CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc-posix
export CXX_x86_64_pc_windows_gnu=x86_64-w64-mingw32-g++-posix
export AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ar
export CRAFT_I18N_REQUIRED=1
cargo build -p themisio --profile "${PROFILE:-quick}" --target x86_64-pc-windows-gnu "$@"
