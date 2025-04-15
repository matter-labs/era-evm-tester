#!/bin/bash

# URL of the tarball
TAR_URL="https://github.com/ethereum/execution-spec-tests/releases/download/v4.2.0/fixtures_develop.tar.gz"

# Target directory
TARGET_DIR="ethereum-fixtures"

# Create the target directory
mkdir -p "$TARGET_DIR"

# Download and extract, stripping the top-level "fixtures" directory
curl -L "$TAR_URL" | tar -xz --strip-components=1 -C "$TARGET_DIR"

echo "Download and extraction complete into '$TARGET_DIR'."
