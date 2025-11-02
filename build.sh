#!/bin/sh

# build libraries
echo "building ppdrive-cli..."
cargo build --release -p ppdrive-cli

echo "building manager..."
cargo build --release -p manager

echo "building ppd-rest..."
cargo build --release -p ppd-rest

echo "building rest-client..."
cargo build --release -p rest-client

echo "building rest-direct..."
cargo build --release -p rest-direct

# remove artifacts
echo "removing artifacts..."
rm target/release/ppd-rest.so
rm target/release/rest-client.so
rm target/release/rest-direct.so

# rename libraries
echo "renaming builds..."
mv target/release/libppd_rest.so target/release/ppd-rest.so
mv target/release/librest_client.so target/release/rest-client.so
mv target/release/librest_direct.so target/release/rest-direct.so
