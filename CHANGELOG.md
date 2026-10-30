# Changelog

## [0.2.0]

- Added `simplex fmt` to format SimplicityHL sources with `simfmt`, which `simplexup` now installs.
- Rewrote fuzz testing to a much more versatile and smart suite.
- Made every command find `Simplex.toml` in the current directory or any of its ancestors.

### fuzz

- Replaced the single-program fuzz engine with `Scenario`, a transaction template where each program `spends` inputs and `locks` outputs.
- Added `bind_arguments` and `bind_witness` to derive values from the case, and `requires_witness` for values the fuzzer also violates to check that the contract rejects them.
- Added `spec` to state when a case must be accepted, rejected or either, read from `FuzzCase`.
- Added transaction `Mutations` of amounts, assets, timelocks and layout, applied to the template's targeted inputs and outputs.
- Added lock and authorization smart checks that redirect locked outputs and replay signatures from a wrong key.
- Added `spending_path` and `storage_capacity` to the program builder, and `random_strategy` and `custom_strategy` to replace the guided default.
- Added a run summary of verdicts, rejections, spec predictions, violations, mutations and smart checks.
- Removed `FuzzableProgram`, `FuzzTransaction`, `ArgsStrategyBuilder`, `ProgramCheck` and `FuzzError`.

### sdk

- Replaced `RequiredSignature` arguments with `Option<NativeSignature>` for native inputs and `Option<ProgramSignature>` for program inputs, built as `ProgramSignature::new(name).at_path(path).tagged(tag)`.
- Made `Signer` sign only inputs that request a native signature and refuse a key that doesn't own a P2WPKH input.
- Made `Signer` derive its keys and addresses once, and reuse one sighash cache per transaction.
- Reduced how often coin selection signs the transaction.
- Fixed coin selection: skip UTXOs listed twice, skip confidential UTXOs for an explicit change target, and keep blinded change that would reveal a blinded amount.
- Fixed fees: report the whole leftover as the fee when change is dropped, and fold explicit change below the 22 sat dust threshold into the fee.
- Made a set locktime enforced by default, by giving inputs without a sequence `ENABLE_LOCKTIME_NO_RBF`.
- Made per-input `sequence` and `locktime` optional, so an explicit `Sequence::MAX` or `LockTime::ZERO` is kept.
- Made `get_utxos` skip outputs the wallet can't unblind, instead of failing.
- Replaced panics with errors for unknown input amounts, amount overflows, invalid fee rates, duplicate inputs, empty issuances, unblinded reissuance tokens, inputs without a witness UTXO and invalid derivation paths.
- Fixed token-only issuance by encoding a zero asset amount as null.
- Added `FinalTransaction::sequence` and `locktime` getters.
- Added `Program::set_storage`, `get_arguments` and `MAX_STORAGE_SLOTS`, and capped storage at 128 slots.
- Made `SimplexProvider::broadcast_transaction` wait until Esplora indexes the transaction.
- Cached the program's CMR, witness types and taproot spend info.

### build

- Refused a dependency `src_dir` that resolves outside its package.
- Refused an `out_dir` that holds the project or its `src_dir`, in both `build` and `clean`.
- Refused source directories that aren't Rust identifiers, before writing any artifact.
- Warned about `.simf` files that don't parse, instead of skipping them silently.

### install

- Resolved tags and branches by their full ref.
- Made git dependency directory names unambiguous.
- Made installs atomic, and reported an incomplete dependency instead of treating it as installed.

### init

- Made `simplex init` optimize dependencies in dev builds, which speeds up tests.

### wasm

- Rejected issuances of zero, reissuance tokens with a zero blinding factor, derivation paths other than `m/n`, and more than 128 extra leaves.

### simplexup

- Verified binaries before installing them and refused versions whose install didn't complete.
- Verified the attestation's signer with `gh` when the GitHub CLI is installed and logged in.
- Installed `--commit` builds as their own version.
- Made `--use` switch every binary to the chosen version.

## [0.1.1]

- Fixed broken dependencies.

## [0.1.0]

- Added fuzz testing: `#[simplex::fuzz]`, `simplex test --fuzz` and the `[test.fuzz]` config section.
- Added the `Guided` (default) and `Random` fuzz strategies.
- Bumped `simplicityhl` to `0.8.0`.

### sdk

- Updated the built-in `Program` to check the pruned program's execution cost against its witness budget during finalization.
- Added `ProgramSchema`, implemented by `include_simf!()`.
- `Signer::sign_tx` and `Signer::sign_witnesses` are now public.
- Removed `ArgumentsTrait` and `WitnessTrait`. `Program::new` and `ProgramInput::new` take `impl Into<Arguments>` and `impl Into<WitnessValues>`.

### build

- Put `smplx-build` code generation behind the default `full` feature.

### config

- Changed the default `[regtest]` and `[test]` mnemonic to `abandon ... about`.

### install

- Installed git dependencies are named with a base58 hash.
- Git dependencies accept a `package` subdirectory.
- Fixed `simplex install` recursing forever on cyclic git dependencies.

### wasm

- `TransactionBuilder` can now reissue an asset by spending a confidential output that holds its reissuance token.
- Issuance and output methods now reject amounts above `i64::MAX` before adding them to a transaction.
- Blinding secrets now accept `value` as a decimal string. JSON numbers above 2^53 - 1 are rejected because JavaScript may serialize them with different digits.

## [0.0.12]

- Added transaction sanity check to the `Signer` to not sign obviously malformed transactions.
- Added dependencies validation step that prevents injection of parameters into the `git` command.
- Made tapleaf "storage" slots exactly 32-byte for onchain compatibility.
- Improved `wasm` API by supporting more `sdk` functionality.
  - Added support for confidential inputs.
  - Added custom derivation path signing (n/m).
  - Added `tapleaf_hash` function to the covenant.
- Fixed several minor bugs.

## [0.0.11]

- Bumped `simplicityhl` dependency to `0.7.2`.
- Added `simplex clean --all` flag to also delete installed dependencies.
- Added an ability to specify a `branch` in the dependencies installation section.
- Added `simplex install std` alias to install the latest `simplicityhl-std` version.
- Added custom Elements network resolution to support networks with dynamic genesis hashes and policy assets.
- Added an ability to sign and inject custom `sighash_all` tags to the `Signer`. Use via `RequiredSignature::WitnessWithMessage`.
- Fixed artifacts generation for dependencies that are deeper than one directory in.

## [0.0.10]

- Added `SECURITY.md` for security reporting guidelines.

### wasm

- Implemented new `wasm` crate with wasm bindings for the sdk.

### sdk

- Put provider-related code under `provider` feature for wasm compatibility.
- Optimized `Program` usage by ~50% via SimplicityHL compilation artifacts caching.
- Added `get_tapleaf_hash` function to `Program` to get the executable tap leaf hash.
- Removed `#[must_use]` from `set_storage_at` function.
- Fixed many bugs in `Program` execution logging. Now traces are rendered correctly if multiple programs are specified or if the execution returns with an error.
- Refactored program's tap storage tree to fold left.
- Refactored `Signer` to be wasm-compatible.
- Added `get_private_key_at` function to `Signer` for LWK compatibility.
- Added configurable `ChangeOutput` to `FinalTransaction` to specify where the transaction change should go.
- Added `set_sequence` and `set_locktime` functions to `FinalTransaction`.

### cli

- Added `metadata.json` file in the `simplex build` output to find the flattened sources and CMRs.
- Added dry run check to the `simplex build` to catch the compilation errors early.
- Added an ability to specify `tag` and `rev` during dependencies installation.
- Fixed an excessive git errors logging during dependencies installation.

## [0.0.9]

- Extended `simplex install` to accept dependency arguments (`simplex install <dep>` or `simplex install <alias>=<dep>`).
- Fixed a bug where electrs may fail to start indexing elements blocks on some linux systems.

## [0.0.8]

- Fixed many issues in the sdk crate regarding the Liquid mainnet network:
  - Changed `NetworkKind` to `Main` when `Liquid` is specified.
  - Changed the default descriptor derivation to `0` from `1`.
  - The change output populated by `Signer` is now confidential instead of being explicit.
  - The `xpub.parent_fingerprint` is now zeroed out before descriptor serialization to be compatible with the Blockstream app.

## [0.0.7]

- Added support for SimplicityHL modules and multifile programs.
- Added `simplex install` command to install external SimplicityHL dependencies.
  - Either via `git clone` or directly as a local dependency.
  - Extended the config to specify the required dependencies.
- Fixed `simplexup` backward compatibility issues.

## [0.0.6]

### Simplex

- Refactored artifacts macro.
- Added new functions to `ProviderTrait`.
  - `fetch_tip_block_hash`.
  - `fetch_block_hash_at_height`.
  - `fetch_block_txids`.
- Implemented `Default` trait for autogenerated witness and arguments.
- Migrated test runner to `nextest`.
  - Refactored `-v` and `-vv` flags to represent various logging verbosity levels.
  - Improved test filtering logic. In order to run non-simplex tests, `--no-simplex` flag has to be specified.
  - Added `--test-threads` flag to indicate the number of threads to be used in tests. Default is `1`.
  - Removed `--nocapture` flag. Use `-v` or `-vv` instead.
- Added transaction cost logging and fixed duplicate simplicity traces.
- Fixed `u1` witness encoding bug.
- Fixed 100% CPU usage on `simplex regtest` execution.
- Rewrote verbosity configuration in both `simplex test` and SDK to check if debug symbols should be included in program compilation.

### Simplexup

- Added `cargo-nextest` binary to the installer.

## [0.0.5]

### Simplex

- Added some documentation to the public simplex functions.
- Implemented "global configuration" singleton in the SDK.
- Added `TxReceipt` object that gets returned upon transaction broadcasting for blocks confirmation convenience.
- Fixed `FinalTransaction` issuance inputs that didn't work correctly with inflation tokens.
- Renamed `with_pub_key` program method to `with_taproot_pubkey`.
- Added `asset` and `amount` methods to the `UTXO` struct.
- Refactored `simplex test` command:
  - Added compatibility for custom test name filters.
  - Added `--target` flag that isolates tests to a specific integration test module.
  - Added `--no-fail-fast` flag to allow tests to keep running even if one fails.
  - Added `--quiet` flag to suppress some simplex logging.
- Refactored `simplex new` command to set up a new project by accepting a directory name.  
- Added `NetworkUtils` to `TestContext` for some "network cheatcodes":
  - The `mine_until_height` function is provided.
- Added a `-v` flag to `simplex test` that logs simplicity pruning traces.
- Fixed `rustfmt` warning on generated artifacts. Now they are skipped.
- Added fixtures for simplex integration tests.

### Simplexup

- Added ability to fetch specific simplex commits.

## [0.0.4]

- Sped up regtest setup 3x times by mining a block after tokens sweep.
- Added basic taproot storage support to `Program`.
- Added support for reissuance tokens (inflation keys) to the `FinalTransaction` builder.
- Implemented nested witness signature injection and parsing.
  - Users can now ask a signer to put a signature under `Either`, `Array`, and `Tuple` types.
- Added the auth and ports config support for the project manifest.
- Fixed a bug where the `build` macro didn't support all `SimplicityHL` types, resulting into a panic.
- Added `get_script_pubkey` and `get_script_hash` function to artifacts.
- Removed `get_program` and `get_program_mut` from artifacts in favor of `as_ref` and `as_mut`.
- Added `new_metadata` function to the `PartialOutput`.
- Added `random_mnemonic` and `random_signer` functions.
- Changed `simplex test` interface to just accept the name (or a pattern) of the tests to run.
  - Only simplex tests should be invoked now.
- Implemented some unit tests.

## [0.0.3]

- Flattened `simplex test` command interface. Removed `run` and `integration` nesting.
- Refactored `Signer` and `Program` interfaces to get rid of unnecessary `.unwrap()` calls.
- Added support for confidential UTXOs.
  - Use `output.with_blinding_key()` to create one.
  - Use `signer.blinding_key()` to fetch a blinding key of a specific signer.
- Renamed `Signer` functions to not use the `wpkh` prefix.
- Renamed `Context` functions to returns a default signer and provider.
- Added `create_signer` function to `Context`.
- Added `UTXO` struct to be used in the entire SDK.
- Refactored `PartialInput` to suport locktime.
- Removed presets from the SDK.
- Handled `ElementsRegtest` in test context instead of panicking.

## [0.0.2]

- Implemented `simplex init` and `simplex clean` commands.
- Added "initial signer bitcoins" to the Simplex configuration.
- Added `fetch_tip_height` and `fetch_tip_timestamp` methods to the providers.
- Added clippy check to CI.
- Fixed regtest not accepting transactions with multiple OP_RETURNs.
- Added `send` method to the signer to be able to quickly send a policy asset.
- Extended `get_wpkh_utxos` method to be able to filter signer's UTXOs on the fly.

## [0.0.1]

- Initial Simplex release!
