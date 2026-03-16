# `simplexup`

Update or revert to a specific Foundry branch with ease.

`foundryup` supports installing and managing multiple versions.

## Installing

```sh
curl -L https://raw.githubusercontent.com/BlockstreamResearch/smplx/master/simplexup/install | bash
```

## Usage

To install the latest stable version:

```sh
simplexup
```

To **install** a specific **version** (in this case the `v0.1.0` version):

```sh
simplex --install nightly
```

To **list** all **versions** installed:

```sh
simplex --list
```

To switch between different versions and **use**:

```sh
simplex --use v0.1.0
```

## Uninstalling

Simplex contains everything in a `.simplex` directory, usually located in `/home/<user>/.simplex/` on Linux, `/Users/<user>/.simplex/` on MacOS and `C:\Users\<user>\.simplex` on Windows where `<user>` is your username.

To uninstall Simplex remove the `.simplex` directory.

Remove Simplex from PATH:

- Optionally Foundry can be removed from editing shell configuration file (`.bashrc`, `.zshrc`, etc.). To do so remove the line that adds Simplex to PATH:

```sh
export PATH="$PATH:/home/user/.simplex/bin"
```
