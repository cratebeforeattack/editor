#!/bin/bash

set -e

BUILD_ROOT=$(dirname $(readlink -f $0))
cd ${BUILD_ROOT}

rm -Rf .temp-release
mkdir -p .temp-release/examples

rm -f ${BUILD_ROOT}/target/x86_64-pc-windows-gnu/release/editor.exe
bash ${BUILD_ROOT}/build_windows.sh
cp ${BUILD_ROOT}/target/x86_64-pc-windows-gnu/release/editor.exe .temp-release/
cp ${BUILD_ROOT}/res/maps/*.cbmap .temp-release/examples/
x86_64-w64-mingw32-strip .temp-release/editor.exe
cp LICENSE .temp-release/
mkdir -p archive/releases/

GIT_TAG=$(git tag --contains HEAD)
GIT_HASH=$(git rev-parse --short HEAD)
cd .temp-release
ZIP_PATH=${BUILD_ROOT}/archive/releases/cba-editor-${GIT_TAG}-${GIT_HASH}.zip
test -f ${ZIP_PATH} && (echo "Zip already exists: $ZIP_PATH"; exit -1)
zip -r -9 ${ZIP_PATH} *
echo "Created ${ZIP_PATH}"
