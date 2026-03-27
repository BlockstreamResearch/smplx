![](https://github.com/user-attachments/assets/c4661df7-6101-4c46-9376-dedaeef8056b)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Tests](https://github.com/BlockstreamResearch/smplx/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/BlockstreamResearch/smplx/workflows/ci.yml)
[![Community](https://img.shields.io/endpoint?color=neon&logo=telegram&label=Chat&url=https%3A%2F%2Ftg.sumanjay.workers.dev%2Fsimplicity_community)](https://t.me/simplicity_community)

# Smplx

**A blazingly-fast, ux-first simplicity development framework.**

## What

Simplex is a Rust-based, comprehensive development framework for [Simplicity](https://github.com/BlockstreamResearch/SimplicityHL) smart contracts, aiming to provide a rich tooling suite for implementing, testing, and deploying smart contract on [Liquid](https://liquid.net/).

- CLI for managing simplicity-based projects.
- SDK with essential simplicity utilities.
- Liquid regtest for local integration testing.
- Extensive framework configuration.

> [!WARNING]
> The framework is at the extremely early stage of development, unforeseen breaking changes and critical bugs are expected.

## Installation

```bash
curl -L https://smplx.simplicity-lang.org | bash
simplexup
```

See the [simplexup manual](simplexup/README.md) for more details.

## Getting started

Create a new Simplex project in a new directory:

```bash
simplex new <name>
```

This scaffolds a complete project with a `Simplex.toml`, `Cargo.toml`, a p2pk contract in `simf/p2pk.simf`, and a working integration test in `tests/p2pk_test.rs`.

To scaffold a full working example instead:

```bash
simplex example basic
```

This creates a `basic/` directory containing the complete basic example project, including several contract examples and an integration test you can run immediately after building.

Alternatively, initialize a Simplex project in the current directory:

```bash
simplex init
```

## Usage

Simplex is a zero-config framework. However, it requires a `simplex.toml` file to exist in the project root. The default configuration is the following:

```toml
# Simplex config

[build]
src_dir = "./simf"
simf_files = ["*.simf"]
out_dir = "./src/artifacts"

[regtest]
mnemonic = "exist carry drive collect lend cereal occur much tiger just involve mean"
bitcoins = 10_000_000

[test]
mnemonic = "exist carry drive collect lend cereal occur much tiger just involve mean"
bitcoins = 10_000_000

[test.esplora]
url = "<esplora url>"
network = "<Liquid, LiquidTestnet, ElementsRegtest>"

[test.rpc]
url = "<rpc url>"
username = "<rpc username>"
password = "<rpc password>"
```

Where:

- `build` (`simplex build` config)
  - `src_dir` - The simplicity contracts source directory.
  - `simf_files` - A glob pattern incidating which contracts are in scope.
  - `out_dir` - The output directory where contracts artifacts are generated.
- `regtest` (`simplex regtest` config)
  - `mnemonic` - The signer's mnemonic regtest will send initial funds to.
  - `bitcoins` - Initial coins available to the signer
- `test` (`simplex test` config)
  - `esplora`
    - `url` - Esplora API endpoint url
    - `network` - Esplora network type (`Liquid`, `LiquidTestnet`, `ElementsRegtest`).
  - `rpc`
    - `url` - Elements RPC endpoint url
    - `username` - Elements RPC username
    - `password` - Elements RPC password
  - `mnemonic` - The signer's mnemonic internal regtest will send initial funds to.
  - `bitcoins` - Initial coins available to the signer

### CLI

Simplex CLI provides the following commands:

- `simplex new <name>` - Creates a new Simplex project in a new `<name>` directory.
- `simplex example <name>` - Scaffolds a complete example project into a new directory (e.g. `simplex example basic`).
- `simplex init` - Initializes a Simplex project in the current directory.
- `simplex config` - Prints the current config.
- `simplex build` - Generates simplicity artifacts.
- `simplex regtest` - Spins up local Electrs + Elements nodes.
- `simplex test` - Runs Simplex tests.
- `simplex clean` - Cleans up generated artifacts.

To view the available options, run the help command:

```bash
simplex -h
```

### Typical workflow

```bash
# Create a new project
simplex new mycontract
cd mycontract

# Build artifacts from .simf contracts
simplex build

# Start a local regtest node and run integration tests
simplex test integration
```

### Using `smplx-std` as a library

`smplx-std` is the Rust library that backs your Simplex project. Add it to `Cargo.toml`:

```toml
[dependencies]
smplx-std = "x.y.z"
```

Everything is re-exported from the `simplex` crate name:

```rust
use simplex::transaction::{FinalTransaction, PartialInput, PartialOutput, ProgramInput, RequiredSignature};
use simplex::utils::tr_unspendable_key;
use simplex::constants::DUMMY_SIGNATURE;
```

#### Building and spending a program

The generated artifacts for each `.simf` contract live in `src/artifacts/` after running `simplex build`. Each contract exposes a typed program struct, an arguments struct, and a witness struct:

```rust
// Generated from simf/p2pk.simf
use my_project::artifacts::p2pk::P2pkProgram;
use my_project::artifacts::p2pk::derived_p2pk::{P2pkArguments, P2pkWitness};
```

Instantiate the program by passing a Taproot internal key and the typed arguments:

```rust
let arguments = P2pkArguments {
    public_key: signer.get_schnorr_public_key().unwrap().serialize(),
};
let program = P2pkProgram::new(tr_unspendable_key(), arguments);
let script = program.get_program().get_script_pubkey(context.get_network()).unwrap();
```

Fund the script by adding it as an output to a `FinalTransaction`:

```rust
let mut ft = FinalTransaction::new(*context.get_network());
ft.add_output(PartialOutput::new(script.clone(), 50, context.get_network().policy_asset()));
let (tx, _) = signer.finalize(&ft).unwrap();
provider.broadcast_transaction(&tx).unwrap();
```

Spend from the script by constructing the witness and calling `add_program_input`. Use `DUMMY_SIGNATURE` as a placeholder — the signer replaces it with a real signature identified by the `RequiredSignature::Witness` name:

```rust
let witness = P2pkWitness { signature: DUMMY_SIGNATURE };
let mut ft = FinalTransaction::new(*context.get_network());
ft.add_program_input(
    PartialInput::new(utxo_outpoint, utxo_txout),
    ProgramInput::new(Box::new(program.get_program().clone()), Box::new(witness)),
    RequiredSignature::Witness("SIGNATURE".to_string()),
).unwrap();
let (tx, _) = signer.finalize(&ft).unwrap();
provider.broadcast_transaction(&tx).unwrap();
```

#### Key types

| Type | Description |
|---|---|
| `FinalTransaction` | Transaction builder — holds inputs and outputs |
| `PartialInput` | A UTXO to spend, identified by outpoint and `TxOut` |
| `PartialOutput` | An output with script, amount, and asset |
| `ProgramInput` | Pairs a compiled Simplicity program with its witness |
| `RequiredSignature` | Tells the signer which witness field to fill (`Witness("NAME")`) |
| `tr_unspendable_key()` | Returns the standard unspendable Taproot internal key used for Simplicity outputs |
| `DUMMY_SIGNATURE` | 64-byte placeholder replaced by the signer at finalization time |

#### Test macro

Annotate integration test functions with `#[simplex::test]` to get an injected `TestContext` wired to the configured regtest or remote network:

```rust
#[simplex::test]
fn my_test(context: simplex::TestContext) -> anyhow::Result<()> {
    let signer = context.get_signer();
    let provider = context.get_provider();
    // ...
    Ok(())
}
```

Run tests with:

```bash
simplex test integration
```

### Examples

Check out the complete project examples in the `examples` directory, or scaffold one locally with `simplex example basic`.

## Contributing

We are open to any mind-blowing ideas! Please take a look at our [contributing guidelines](CONTRIBUTING.md) to get involved.

## Future work

- [x] Complete `simplex init`, `simplex new`, `simplex example`, and `simplex clean` commands.
- [ ] SDK support for confidential assets, taproot signer, and custom witness signatures.
- [ ] Local regtest 10x speedup.
- [ ] Regtest cheat codes.
- [ ] Browser compatibility.
- [ ] Simplicity dependencies management once the language adds [support for modules](https://github.com/BlockstreamResearch/SimplicityHL/issues/155).
- [ ] Comprehensive documentation.

## License

The framework is released under the MIT License.
