#/usr/bin/bash
set -euo pipefail
IFS=$'\n\t'


pushd xtask
cargo run
popd

cargo build
