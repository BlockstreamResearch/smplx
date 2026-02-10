# Simpelex HL Core

This crate provides useful utilities for working with Simplicity on Elements.

- `blinder.rs` — derives deterministic blinder keypair from a "public secret"
- `constants.rs` — Liquid network constants (policy asset IDs, genesis hashes)
- `explorer.rs` — explorer API utilities (behind `explorer` feature)
- `runner.rs` — program execution helpers with logging
- `scripts.rs` — P2TR address creation, Taproot control block, and asset entropy utilities
- `lib.rs` — P2PK program helpers and transaction finalization

Consider this more like a test helper tool rather than a production-ready version.

## License

Dual-licensed under either of:
- Apache License, Version 2.0 (Apache-2.0)
- MIT license (MIT)

at your option.
