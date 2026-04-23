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

Add `smplx-std` dependency to cargo:

```bash
cargo add --dev smplx-std
```

Optionally, initialize a new project:

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
rpc_port = 18443
esplora_port = 3000
rpc_user = "user"
rpc_password = "password"

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
  - `simf_files` - A glob pattern indicating which contracts are in scope.
  - `out_dir` - The output directory where contracts artifacts are generated.
- `regtest` (`simplex regtest` config)
  - `mnemonic` - The signer's mnemonic regtest will send initial funds to.
  - `bitcoins` - Initial coins available to the signer.
  - `rpc_port` - The port Elements regtest node will listen on.
  - `esplora_port` - The port Electrs will listen on.
  - `rpc_user` - Elements regtest RPC username.
  - `rpc_password` - Elements regtest RPC password.
- `test` (`simplex test` config)
  - `mnemonic` - The signer's mnemonic internal regtest will send initial funds to.
  - `bitcoins` - Initial coins available to the signer.
  - `esplora`
    - `url` - Esplora API endpoint url.
    - `network` - Esplora network type (`Liquid`, `LiquidTestnet`, `ElementsRegtest`).
  - `rpc`
    - `url` - Elements RPC endpoint url.
    - `username` - Elements RPC username.
    - `password` - Elements RPC password.

### CLI

Simplex CLI provides the following commands:

- `simplex init` - Initializes a new Simplex project.
- `simplex config` - Prints the current config.
- `simplex build` - Generates simplicity artifacts.
- `simplex regtest` - Spins up local Electrs + Elements nodes.
- `simplex test` - Runs Simplex tests.
- `simplex clean` - Cleans up generated artifacts.

To view the available options, run the help command:

```bash
simplex -h
```

### Example

Check out the complete project examples in the `examples` directory to learn more.

## Contributing

We are open to any mind-blowing ideas! Please take a look at our [contributing guidelines](CONTRIBUTING.md) to get involved.

## Future work

- [x] Complete `simplex init` and `simplex clean` tasks.
- [x] Simplicity storage compatibility.
- [ ] SDK support for confidential assets, taproot signer, and custom witness signatures.
- [ ] Local regtest 10x speedup.
- [ ] Regtest cheat codes.
- [ ] Browser compatibility.
- [ ] Simplicity dependencies management once the language adds [support for modules](https://github.com/BlockstreamResearch/SimplicityHL/issues/155).
- [ ] Comprehensive documentation.

## License

The framework is released under the MIT License.
