#!/usr/bin/env bash
# Builds the browser-loadable Simplex SDK package into crates/wasm/pkg.
#
# Uses wasm-pack instead of wasm-bindgen directly.
#
# Usage: crates/wasm/build.sh [bundler|nodejs|web]   (default: bundler)

set -euo pipefail

TARGET_KIND="${1:-bundler}"
CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(cd "${CRATE_DIR}/../.." && pwd)"

# A wasm-capable C compiler is required. Apple's system clang has no WebAssembly
# backend, and without this the build fails inside secp256k1-sys and simplicity-sys
# with "unable to create target", which points at the crates and misleads.
if [ -z "${CC_wasm32_unknown_unknown:-}" ]; then
	for candidate in /opt/homebrew/opt/llvm/bin/clang /usr/local/opt/llvm/bin/clang /usr/bin/clang; do
		if [ -x "${candidate}" ] && "${candidate}" -print-targets 2>/dev/null | grep -q wasm32; then
			export CC_wasm32_unknown_unknown="${candidate}"

			candidate_ar="$(dirname "${candidate}")/llvm-ar"

			if [ ! -x "${candidate_ar}" ]; then
				candidate_ar="$(command -v llvm-ar || true)"
			fi

			if [ -z "${candidate_ar}" ]; then
				candidate_ar="$(ls -1 "$(dirname "${candidate}")"/llvm-ar-* 2>/dev/null | sort -V | tail -1 || true)"
			fi

			if [ -z "${candidate_ar}" ]; then
				echo "error: found ${candidate} but no llvm-ar beside it or on PATH." >&2
				echo "       Install LLVM's binutils, or set AR_wasm32_unknown_unknown yourself." >&2
				exit 1
			fi

			export AR_wasm32_unknown_unknown="${candidate_ar}"
			break
		fi
	done
fi

if [ -z "${CC_wasm32_unknown_unknown:-}" ]; then
	echo "error: no C compiler with a WebAssembly backend was found." >&2
	echo "       Install LLVM (brew install llvm) and set CC_wasm32_unknown_unknown" >&2
	echo "       and AR_wasm32_unknown_unknown to its clang and llvm-ar." >&2
	exit 1
fi

if ! command -v wasm-pack >/dev/null 2>&1; then
	echo "error: wasm-pack is not installed." >&2
	echo "       Install it with: cargo install wasm-pack" >&2
	exit 1
fi

cd "${WORKSPACE_DIR}"

wasm-pack build crates/wasm --target "${TARGET_KIND}" --release --out-dir pkg

echo
echo "Built ${CRATE_DIR}/pkg (${TARGET_KIND}):"
ls -l "${CRATE_DIR}/pkg"
