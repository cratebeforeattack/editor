#!/bin/bash

set -e

ROOT=$(dirname $(readlink -f $0))

pushd ${ROOT}
cargo run --release
popd
