.PHONY: build test fmt check clean

# Build all contracts in the workspace as optimized WASM artifacts
build:
	stellar contract build

# Run all unit tests across the workspace
test:
	cargo test

# Format all source files
fmt:
	cargo fmt --all

# Check formatting and compilation without producing artifacts
check:
	cargo fmt --all -- --check
	cargo check

# Remove build artifacts
clean:
	cargo clean
