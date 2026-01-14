#!/bin/bash
ulimit -n 4096
cargo run --manifest-path engine/Cargo.toml -p cleoselene -- games/fighting-example/main.lua --port 3425
