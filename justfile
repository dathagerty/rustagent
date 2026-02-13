# Run all tests
test *args:
    cargo test {{args}}

# Build CLI without web UI
build:
    cargo build

# Build CLI with embedded web UI
build-ui: web-build
    cargo build --features bundle-ui

# Release build without web UI
release:
    cargo build --release

# Release build with embedded web UI
release-ui: web-build
    cargo build --release --features bundle-ui

# Build the web UI (prerequisite for bundle-ui builds)
web-build:
    cd web && bun run build

# Run the web UI dev server
web-dev:
    cd web && bun run dev

# Run cargo clippy
lint:
    cargo clippy

# Format code
fmt:
    cargo fmt
