#!/bin/bash

set -e

# Make sure that we have right target installed
rustup target add x86_64-pc-windows-gnu

BUILD_ROOT=$(dirname $(readlink -f $0))

cd ${BUILD_ROOT} 
rm -f res.zip
cargo build --target=x86_64-pc-windows-gnu --release
x86_64-w64-mingw32-strip ${BUILD_ROOT}/target/x86_64-pc-windows-gnu/release/editor.exe
