# Simplex project and validation

Target toolchain: Simplex 0.2.0 / SimplicityHL 0.8.0. The binary is `simplex`, although the upstream repository is named `smplx` and Rust packages are `smplx-*`.

## Minimal offline project

`Simplex.toml` (capital S; exact filename used by the pinned CLI):

```toml
[build]
src_dir = "./simf"
simf_files = ["*.simf"]
out_dir = "./src/artifacts"
```

Place the spending program in `simf/contract.simf`. Run `simplex build` in the project directory. For valid discovered source files, this command resolves/typechecks the source, compiles using default parameters, flattens imports, and emits Rust bindings plus `src/artifacts/metadata.json`. No chain daemon or network is required for a self-contained program. The metadata CMR uses default arguments, and is not the deployed CMR of a contract with real public keys. Instantiate with actual arguments to derive its deployment commitment/address.

**Check artifact coverage, not just the exit code.** Source discovery warns about files that its parser rejects; these warnings do not prove that every intended contract was built. Confirm that `metadata.json` contains every intended entrypoint and that its matching Rust binding exists. Missing coverage needs investigation even when the command exits successfully. Use the bundled checker for a self-contained source if the build output is insufficient; it does not resolve project imports.

If you need a separate diagnostic for a self-contained target, keep that target in place and run the bundled [source checker](../assets/check_source.py). From your project, run `python3 ABSOLUTE_SKILL_ROOT/assets/check_source.py simf/contract.simf` substituting the directory returned by `simplex skill`. Respect any launcher restrictions in the environment. It creates a standalone `.simplex-diagnostics` Cargo package, uses the pinned `simplex::simplicityhl` compiler offline, and prints its real grammar/type error. It does not depend on missing generated artifacts from your main project. Cached `smplx-std = 0.2.0` and Rust tools are required. The checker supports a single self-contained source; projects with imports need their configured module resolver or flattened source. Fix the reported source error, rerun the checker, then rerun `simplex build` and confirm the intended source is covered. A checker success is not a build or an execution test. Replacing the target with a trivial smoke program does not validate the requested contract.

When only the Simplex CLI is permitted, create a normal Rust diagnostic test and run it with `simplex test --no-simplex --show-output`:

```rust
#[test]
fn source_compiles() {
    use simplex::simplicityhl::{TemplateAst, UnstableFeatures};
    use simplex::simplicityhl::ast::ElementsJetHinter;
    TemplateAst::new_with_unstable(
        include_str!("../simf/contract.simf"),
        &UnstableFeatures::all(), Box::new(ElementsJetHinter),
    ).unwrap_or_else(|error| panic!("{error}"));
}
```

This invokes the same compiler through the SDK and exposes its source error in the test output; it is not an execution test. If the library's `pub mod artifacts;` prevents the diagnostic test from compiling because artifacts are missing, temporarily remove that declaration for this diagnostic, then restore it after a successful `simplex build`. Keep the actual target source and verify metadata coverage before reporting a completed project. CLI restrictions still apply: no direct Cargo or other compiler command is needed for this diagnostic.

`simplex build` does not require a Cargo project when only generating artifacts. For a Rust SDK project, add these files:

```toml
# Cargo.toml
[package]
name = "contract-example"
version = "0.1.0"
edition = "2024"

[dependencies]
smplx-std = "=0.2.0"
```

```rust
// src/lib.rs
pub mod artifacts;
```

The Rust crate name exposed by `smplx-std` is `simplex`. Inspect the generated binding to get the actual program/argument/witness struct names and fields; the generator derives them from the source filename and witness names. Do not guess them. The binding embeds a flattened `.simf` program, supports instantiation with arguments, and supplies typed witness conversion.

`simplex init NAME` creates a Rust SDK project and fetches crates.io package-version metadata. If the provided launcher specifically permits this automatic initializer request, use it to create the project; this does not authorize external research. For an environment prohibiting even package metadata requests, manually create the files above and report that limitation. Self-contained jet-based predicates need no SimplicityHL `std` module dependency.

The initializer already writes `pub mod artifacts;` in `src/lib.rs`; keep that single declaration instead of appending another. The initializer supplies a sample `simf/p2pk.simf` and a dummy `tests/p2pk_test.rs` with `todo!()`. Remove or replace unused scaffolding. Run the build before cargo checks/tests so `src/artifacts` exists. Pin `smplx-std = "=0.2.0"`; the Rust crate exposed by that package is `simplex`.

## Imports when needed

Only exported symbols can be imported. Use `pub fn helper(...) { ... }` in a module file and `use crate::module::helper;` in the caller. A subdirectory module requires the matching `mod.simf`. Dependency imports use configured remappings in `[dependencies]`; installation happens separately before building. If using `std`, pin the SimplicityHL standard library revision and install it before going offline. Avoid dependencies for a handful of signature/hash/lock checks.

## Testing levels

- Build: `simplex build`. Confirm exit status, fresh artifacts, and an entry in metadata for every intended contract. Never accept an empty `sources` map for a project containing an entrypoint.
- Rust bindings: `cargo check --offline` with pre-provisioned dependencies. Missing caches are an environment issue, not evidence that contract syntax is wrong.
- Execution: instantiate a `simplicityhl::TemplateAst` with `Arguments`, resolve its `WitnessValues`, build an `ElementsEnv` with the spending transaction/UTXOs/CMR/control block/genesis hash, then execute the satisfied redeem node with `BitMachine`. Real Schnorr signatures must be calculated over that environment's `sig_all_hash`.
- Integration: Rust functions marked `#[simplex::test]` receive a `simplex::TestContext`; `simplex test` orchestrates local Elements/Electrs and requires the corresponding binaries/cargo dependencies. It may start services and is distinct from the offline build.
- Offline behavior tests: ordinary Rust `#[test]` functions can use the supplied fixture and run under `simplex test --no-simplex --show-output`. This still uses Simplex's real test runner and compiles the generated SDK bindings. Read [testing.md](testing.md) for a complete fixture API.

Test valid and invalid signatures against actual transaction hashes. For multisig, enumerate all presence subsets; verify all present signatures, including a deliberately invalid extra signature. For a refund branch, test delay below/at/above the value, disabled/time sequences, old transaction versions, and unrelated delayed inputs. Document the absence of chain-height upper bounds rather than fabricating a test that claims to enforce one.
