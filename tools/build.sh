#!/usr/bin/env bash

echo "Building Windows..."
cargo build -q --target=x86_64-pc-windows-gnu --release

echo "Building Linux..."
cargo build -q --release
