#!/bin/bash
ulimit -n 4096
cargo run --manifest-path engine/Cargo.toml -p cleoselene -- games/astro-maze/main.lua --port 3425 --debug-mcp
