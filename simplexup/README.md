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
simplexup --install 0.1.0
```

To list all versions installed:

```bash
simplexup --list
```

To switch between different versions:

```bash
simplexup --use 0.1.0
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
