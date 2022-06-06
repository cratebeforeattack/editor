#!/bin/bash

set -e

ROOT=$(dirname $(readlink -f $0))

${ROOT}/target/debug/editor
