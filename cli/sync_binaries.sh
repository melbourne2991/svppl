#!/bin/bash

# Define the esbuild version and the architectures
ESBUILD_VERSION="0.19.9"
ARCHITECTURES=("darwin-x64" "darwin-arm64" "linux-x64" "linux-arm64")

rm -rf ./third_party/*

# Loop through each architecture
for ARCH in "${ARCHITECTURES[@]}"; do
    # Create a directory for the architecture
    mkdir -p "./third_party/${ARCH}/esbuild"

    # Form the download URL
    URL="https://registry.npmjs.org/@esbuild/${ARCH}/-/${ARCH}-${ESBUILD_VERSION}.tgz"

    # Download and extract the tarball into the respective directory
    curl -L "$URL" | tar -xz -C "./third_party/${ARCH}/esbuild" --strip-components=1
done

# Echo usage instructions
echo "Usage: ./<arch>/esbuild/package/bin/esbuild [options] [entry points]"
