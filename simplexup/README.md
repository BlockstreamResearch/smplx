# simplexup

Install, update, and manage Simplex with ease.

## Installing

Download simplexup:

```bash
curl -L https://raw.githubusercontent.com/BlockstreamResearch/smplx/master/simplexup/install | bash
```

## Usage

To install the latest stable Simplex version:

```bash
simplexup
```

To install a specific version (in this case the `v0.1.0` version):

```bash
simplexup --install v0.1.0
```

To install from a specific commit:

```bash
simplexup --commit b122e8d32911c96da47e457a97046269df28c0ca
```

To list all versions installed:

```bash
simplexup --list
```

To switch between different versions:

```bash
simplexup --use v0.1.0
```

To update `simplexup`:

```bash
simplexup --update
```

## Uninstalling

Simplex contains everything in a `.simplex` directory located in `/home/<user>/.simplex/` on Linux and `/Users/<user>/.simplex/` on Macos, where `<user>` is your username.

To uninstall Simplex, just remove the `.simplex` directory.

Optionally remove Simplex from PATH:

```bash
export PATH="$PATH:/home/user/.simplex/bin"
```

## Disclaimer

Simplicity simplified.
