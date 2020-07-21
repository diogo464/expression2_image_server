#!/bin/sh
cargo build --release
cp ../target/release/expression2_image_server .
cp -r ../images images