# Changelog

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
