# Format Rust files in a working directory.
fmt working_directory:
    #!/usr/bin/env bash
    set -euo pipefail

    cd -- "{{ working_directory }}"
    cargo fmt --all

# Check Rust formatting in a working directory.
fmtcheck working_directory:
    #!/usr/bin/env bash
    set -euo pipefail

    cd -- "{{ working_directory }}"
    cargo fmt --all --check

# Run Clippy in a working directory.
lint working_directory:
    #!/usr/bin/env bash
    set -euo pipefail

    cd -- "{{ working_directory }}"
    cargo clippy --workspace --all-targets --all-features -- --deny warnings

# Build all workspace crates and features in a working directory.
build working_directory:
    #!/usr/bin/env bash
    set -euo pipefail

    cd -- "{{ working_directory }}"
    cargo build --workspace --all-features

# Clean Cargo build output in a working directory.
clean working_directory:
    #!/usr/bin/env bash
    set -euo pipefail

    cd -- "{{ working_directory }}"
    cargo clean

# Run workspace tests in a working directory.
test working_directory:
    #!/usr/bin/env bash
    set -euo pipefail

    cd -- "{{ working_directory }}"
    cargo test --workspace --all-features --no-fail-fast --verbose

# Run workspace tests and long UI tests in a working directory.
test_ui working_directory:
    #!/usr/bin/env bash
    set -euo pipefail

    cd -- "{{ working_directory }}"
    RUN_UI_TESTS=true cargo test --workspace --all-features --no-fail-fast --verbose

# Check the WASM crate in a working directory.
build_wasm working_directory:
    #!/usr/bin/env bash
    set -euo pipefail

    cd -- "{{ working_directory }}"
    cargo check --package smplx-wasm --target wasm32-unknown-unknown

# Check all feature combinations in a working directory.
build_features working_directory:
    #!/usr/bin/env bash
    set -euo pipefail

    cd -- "{{ working_directory }}"
    cargo hack check --feature-powerset --no-dev-deps

# Run the standard cargo-hack check in a working directory.
check_hack working_directory:
    #!/usr/bin/env bash
    set -euo pipefail

    cd -- "{{ working_directory }}"
    cargo hack check

# Check dependency bans, licenses, and sources in a working directory.
check_deny working_directory:
    #!/usr/bin/env bash
    set -euo pipefail

    cd -- "{{ working_directory }}"
    cargo deny check bans licenses sources

# Show Cargo and Rust compiler versions in a working directory.
versions working_directory:
    #!/usr/bin/env bash
    set -euo pipefail

    cd -- "{{ working_directory }}"
    cargo --version
    rustc --version
