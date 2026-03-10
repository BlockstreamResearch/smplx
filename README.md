![](https://github.com/user-attachments/assets/7d7ca314-b706-47b3-a0be-2d64fc409fab)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

# Simplex

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
cargo install --path ./crates/cli
```

*The proper installer will be provided soon.*

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

[test]
mnemonic = "exist carry drive collect lend cereal occur much tiger just involve mean"

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
- `test` (`simplex test` config)
  - `esplora`
    - `url` - Esplora API endpoint url
    - `network` - Esplora network type (`Liquid`, `LiquidTestnet`, `ElementsRegtest`).
  - `rpc`
    - `url` - Elements RPC endpoint url
    - `username` - Elements RPC username
    - `password` - Elements RPC password

### CLI

Simplex CLI provides the following commands:

- `simplex init` - Initializes a Simplex project.
- `simplex config` - Prints the current config.
- `simplex build` - Generates simplicity artifacts.
- `simplex regtest` - Spins up local Electrs + Elements nodes.
- `simplex test` - Runs Simplex tests.
- `simplex clean` - Cleans up the project.

To view the available options, run the help command:

```bash
simplex -h
```

### Example

Check out the complete project examples in the `examples` directory to learn more.

## Future work

- [ ] SDK support for confidential assets, taproot signer, and custom witness signatures.
- [ ] `simplex init` and `simplex clean` tasks.
- [ ] Proper installation scripts.
- [ ] Simplicity dependencies management once the language adds [support for modules](https://github.com/BlockstreamResearch/SimplicityHL/issues/155).
- [ ] Comprehensive documentation.

## License

The framework is released under the MIT License.
