# Format Rust files in a project directory
fmt_inner project_dir:
    #!/usr/bin/env bash
    set -euo pipefail

    cd "{{project_dir}}"
    cargo fmt --all

# Format workspace crates
fmt_crate: (fmt_inner ".")

# Format the basic example
fmt_basic_example: (fmt_inner "examples/basic")

# Format fixtures
fmt_fixtures: (fmt_inner "fixtures")

# Format code
fmt: fmt_crate fmt_basic_example fmt_fixtures

# Check Rust formatting in a project directory
fmtcheck_inner project_dir:
    #!/usr/bin/env bash
    set -euo pipefail

    cd "{{project_dir}}"
    cargo fmt --all -- --check

# Check formatting for workspace crates
fmtcheck_crate: (fmtcheck_inner ".")

# Check formatting for the basic example
fmtcheck_basic_example: (fmtcheck_inner "examples/basic")

# Check formatting for fixtures
fmtcheck_fixtures: (fmtcheck_inner "fixtures")

# Check if code is formatted
fmtcheck: fmtcheck_crate fmtcheck_basic_example fmtcheck_fixtures

# Run Clippy in a project directory
lint_inner project_dir:
    #!/usr/bin/env bash
    set -euo pipefail

    cd "{{project_dir}}"
    cargo clippy --workspace --all-targets --all-features -- --deny warnings

# Run Clippy for workspace crates
lint_crate: (lint_inner ".")

# Run Clippy for the basic example
lint_basic_example: (lint_inner "examples/basic")

# Run Clippy for fixtures
lint_fixtures: (lint_inner "fixtures")

# Run code linter
lint: lint_crate lint_basic_example lint_fixtures

# Run unit tests
test:
    cargo test --workspace --all-features --no-fail-fast --verbose

# Run unit tests and long UI ones
test_ui:
    RUN_UI_TESTS=true cargo test --workspace --all-features --no-fail-fast --verbose

# Build all workspace crates with all features in a project directory
build_inner project_dir:
    #!/usr/bin/env bash
    set -euo pipefail

    cd "{{project_dir}}"
    cargo build --workspace --all-features

# Build workspace crates
build_crate: (build_inner ".")

# Build the basic example
build_basic_example: (build_inner "examples/basic")

# Build fixtures
build_fixtures: (build_inner "fixtures")

# Build all workspace crates with all features
build: build_crate build_basic_example build_fixtures

# Build `smplx-wasm` for the WASM target
build_wasm:
    cargo check --package smplx-wasm --target wasm32-unknown-unknown

# Install and build Simplex dependencies for a project directory
build_simplex_deps project_dir:
    #!/usr/bin/env bash
    set -euo pipefail

    cd "{{project_dir}}"
    test_simplex install
    test_simplex build

# Runs simplex tests in `fixtures` directory
[working-directory: 'fixtures']
check_fixtures: (build_simplex_deps "fixtures")
    test_simplex test

# Runs simplex tests in `examples/basic` directory
[working-directory: 'examples/basic']
check_basic_example: (build_simplex_deps "examples/basic")
    test_simplex test

# Build code with all feature combinations
build_features:
    cargo hack check --feature-powerset --no-dev-deps

# Run the standard cargo-hack check used in CI
check_hack:
    cargo hack check

# Check for `cargo deny`
check_deny:
    cargo deny check bans licenses sources

# Check code (CI)
check:
    cargo --version
    rustc --version
    just fmtcheck
    just lint
    just build
    just build_features
    just check_hack
    just check_deny
    just test
    just test_ui
    just build_wasm
    just check_fixtures
    just check_basic_example

# Installs simplex from local crate and moves it to the default `~/.cargo/bin` dir
#  with name `test_simplex`
install_local_simplex:
    #!/usr/bin/env bash
    set -euo pipefail

    install_root="$(mktemp -d)" || exit 1
    cargo_bin_dir="${CARGO_HOME:-$HOME/.cargo}/bin"
    trap 'rm -rf "$install_root"' EXIT

    cargo install \
        --path ./crates/cli \
        --bin simplex \
        --root "$install_root" && mkdir -p "$cargo_bin_dir" && install -m 755 \
        "$install_root/bin/simplex" \
        "$cargo_bin_dir/test_simplex"

# Install simplex helper binaries
[working-directory: 'simplexup']
install_simplex:
    #!/usr/bin/env bash
    set -euo pipefail

    ./simplexup

# Install helper binaries, which are used in our check
install:
    cargo install cargo-hack
    cargo install cargo-deny
    just install_local_simplex
    just install_simplex

# Clean `fixtures` directory from temporary files
[working-directory: 'fixtures']
clean_fixtures:
    test_simplex clean
    rm -rf target

# Clean `examples/basic` directory from temporary files
[working-directory: 'examples/basic']
clean_examples_basic:
    test_simplex clean
    rm -rf target

# Remove temporary `test_simplex` binary
clean_test_bin:
    #!/usr/bin/env bash
    set -euo pipefail

    cargo_bin_dir="${CARGO_HOME:-$HOME/.cargo}/bin"
    rm "$cargo_bin_dir/test_simplex"

# Remove all temporary files
clean:
    rm -rf target

    just clean_fixtures
    just clean_examples_basic
    just clean_test_bin
