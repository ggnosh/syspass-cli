#!/usr/bin/env bash

echo "Building Windows"
cargo build --target=x86_64-pc-windows-gnu --release

echo "Building Linux"
cargo build --release

echo "Building process completed, press any key to exit"
read
