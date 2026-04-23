# Changelog

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
