# smplx-sdk

The `smplx-sdk` crate provides a comprehensive set of tools in Rust to interact with Simplicity smart contracts on
Liquid networks.
It simplifies building, signing, and broadcasting transactions, as well as interacting with Simplicity programs.

## Features

- **Signer**: Securely parse BIP39 mnemonics, manage keys, sign transactions, and derive Blinding/Confidential and
  unconfidential addresses.
- **Providers**: Easily connect to existing Element nodes via RPC or Esplora APIs to query UTXOs and broadcast
  transactions.
- **Transaction Toolkit**: High-level builder abstractions over `FinalTransaction`, `TxReceipt`, `UTXO`, `PartialInput`,
  and `PartialOutput`.
- **Simplicity Programs**: Effortlessly load, deploy, and interact with your compiled Simplicity (`.simf`) smart
  contracts with Rust.
-

## Quick Start

Read [simplex/README.md](../../README.md).

# Supported platforms

Simplex currently guarantees support for the following platforms:

* Linux x86-64
* Darwin arm64

Simplex will continue to support these platforms in the future. However,
future releases may expand further support for different kinds of platforms.