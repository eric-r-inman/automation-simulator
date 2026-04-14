# `just` ≡ one-shot "does the whole thing work?"
# Builds the Elm bundle, then starts the server pre-loaded with
# the example property + catalog on http://127.0.0.1:3737/.
# Ctrl+C to stop.
default: demo

# Shortest alias for the default demo flow.
up: demo

# Build both Rust and Elm.
build: build-elm build-rust

# Build the Elm frontend.
build-elm:
    cd frontend && elm make src/Main.elm --output public/elm.js

# Build all Rust workspace crates.
build-rust:
    cargo build --workspace

# Run all tests (Elm compile check + Rust test suite).
test: build-elm test-rust

# Run the Rust test suite.
test-rust:
    cargo test --workspace

# Format every file treefmt knows about.
fmt:
    treefmt

# Build Elm then run via cargo, forwarding all arguments.
run *args: build-elm
    cargo run {{args}}

# Run the local demo: build the Elm bundle, then start the server
# pre-loaded with the example property + catalog.  Open
# http://127.0.0.1:3737/ in a browser to interact; Ctrl+C stops the
# server.
demo: build-elm
    @echo ""
    @echo "=== automation-simulator ==="
    @echo "Open http://127.0.0.1:3737/ in your browser; Ctrl+C to stop."
    @echo ""
    cargo run -p automation-simulator-server -- \
      --base-url http://localhost:3737 \
      --frontend-path frontend/public \
      --listen 127.0.0.1:3737 \
      --property-path data/properties/example-property.toml \
      --catalog-path data/catalog
