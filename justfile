set minimum-version := '1.55.0'

root := justfile_directory()
basic_example := root / 'examples/basic'
fixtures := root / 'fixtures'

# Run reusable Rust operations in a selected workspace.
mod rust 'scripts/just/rust.just'

# Run Simplex CLI operations in a selected project.
mod simplex 'scripts/just/simplex.just'

# Format all Rust projects.
fmt: (rust::fmt root) (rust::fmt basic_example) (rust::fmt fixtures)

# Check formatting for all Rust projects.
fmtcheck: (rust::fmtcheck root) (rust::fmtcheck basic_example) (rust::fmtcheck fixtures)

# Run Clippy for all Rust projects.
lint: (rust::lint root) (rust::lint basic_example) (rust::lint fixtures)

# Run workspace unit tests.
test: (rust::test root)

# Run workspace unit tests and long UI tests.
test_ui: (rust::test_ui root)

# Build all Rust projects with all features.
build: (rust::build root) (rust::build basic_example) (rust::build fixtures)

# Build `smplx-wasm` for the WASM target.
build_wasm: (rust::build_wasm root)

# Install and build Simplex dependencies for all projects.
[arg('simplex_bin', long='real', value='simplex', help='Use the installed simplex binary')]
build_simplex_deps simplex_bin='test_simplex': (simplex::build basic_example simplex_bin) (simplex::build fixtures simplex_bin)

# Run Simplex tests in the fixtures project.
[arg('simplex_bin', long='real', value='simplex', help='Use the installed simplex binary')]
check_fixtures simplex_bin='test_simplex': (simplex::build fixtures simplex_bin) (simplex::test fixtures simplex_bin)

# Run Simplex fuzz tests in the fixtures project.
[arg('simplex_bin', long='real', value='simplex', help='Use the installed simplex binary')]
check_fuzz simplex_bin='test_simplex': (simplex::build fixtures simplex_bin) (simplex::test_fuzz fixtures simplex_bin)

# Run Simplex tests in the basic example.
[arg('simplex_bin', long='real', value='simplex', help='Use the installed simplex binary')]
check_basic_example simplex_bin='test_simplex': (simplex::build basic_example simplex_bin) (simplex::test basic_example simplex_bin)

# Build code with all feature combinations.
build_features: (rust::build_features root)

# Run the standard cargo-hack check used in CI.
check_hack: (rust::check_hack root)

# Check dependency bans, licenses, and sources.
check_deny: (rust::check_deny root)

# Run all local checks.
[arg('fuzz', long, value='true', help='Run Simplex fuzz tests in fixtures')]
[arg('real', long, value='--real', help='Use the installed simplex binary')]
check real='' fuzz='false':
    just rust::versions "{{ root }}"
    just build_simplex_deps {{ real }}
    just build
    just fmtcheck
    just lint
    just build_features
    just check_hack
    just check_deny
    just test_ui
    just build_wasm
    just check_fixtures {{ real }}
    just check_basic_example {{ real }}
    {{ if fuzz == 'true' { 'just check_fuzz ' + real } else { "" } }}

# Install Rust tools used by local checks.
[private]
install_rust_tools:
    #!/usr/bin/env bash
    set -euo pipefail

    cargo install cargo-hack
    cargo install cargo-deny

# Install the local Simplex CLI as `test_simplex`.
[private]
install_local_simplex:
    #!/usr/bin/env bash
    set -euo pipefail

    install_root="$(mktemp -d)"
    cargo_bin_dir="${CARGO_HOME:-$HOME/.cargo}/bin"
    trap 'rm -rf "$install_root"' EXIT

    cargo +stable install \
        --path ./crates/cli \
        --bin simplex \
        --root "$install_root"
    mkdir -p "$cargo_bin_dir"
    install -m 755 "$install_root/bin/simplex" "$cargo_bin_dir/test_simplex"

# Install the Simplex helper binaries.
[private]
[working-directory('simplexup')]
install_simplex:
    #!/usr/bin/env bash
    set -euo pipefail

    ./simplexup

# Install all tools used by local checks.
install: install_rust_tools install_local_simplex install_simplex

# Remove all temporary build and generated files.
[arg('simplex_bin', long='real', value='simplex', help='Use the installed simplex binary')]
clean simplex_bin='test_simplex': (clean_simplex simplex_bin) clean_cargo clean_test_bin

# Clean Simplex-generated artifacts from all projects.
[arg('simplex_bin', long='real', value='simplex', help='Use the installed simplex binary')]
clean_simplex simplex_bin='test_simplex': (simplex::clean fixtures simplex_bin) (simplex::clean basic_example simplex_bin)

# Clean generated artifacts and build output from fixtures.
[arg('simplex_bin', long='real', value='simplex', help='Use the installed simplex binary')]
clean_fixtures simplex_bin='test_simplex': (simplex::clean fixtures simplex_bin)
    #!/usr/bin/env bash
    set -euo pipefail

    rm -rf -- "{{ fixtures }}/target"

# Clean generated artifacts and build output from the basic example.
[arg('simplex_bin', long='real', value='simplex', help='Use the installed simplex binary')]
clean_examples_basic simplex_bin='test_simplex': (simplex::clean basic_example simplex_bin)
    #!/usr/bin/env bash
    set -euo pipefail

    rm -rf -- "{{ basic_example }}/target"

# Remove the temporary `test_simplex` binary.
clean_test_bin:
    #!/usr/bin/env bash
    set -euo pipefail

    cargo_bin_dir="${CARGO_HOME:-$HOME/.cargo}/bin"
    rm -f -- "$cargo_bin_dir/test_simplex"

# Clean Cargo build output from all projects.
clean_cargo: clean_cargo_fixtures clean_cargo_examples_basic clean_cargo_root

# Clean Cargo build output from fixtures.
[private]
clean_cargo_fixtures: (rust::clean fixtures)

# Clean Cargo build output from the basic example.
[private]
clean_cargo_examples_basic: (rust::clean basic_example)

# Clean Cargo build output from the root workspace.
[private]
clean_cargo_root: (rust::clean root)
