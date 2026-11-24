# Executable offline tests

Simplex can run ordinary Rust tests with `simplex test --no-simplex --show-output`. This launches its bundled nextest runner and compiles the real generated bindings. It does not start a regtest node for ordinary `#[test]` functions. `#[simplex::test]` functions instead receive a network `TestContext` and normally start Elements/Electrs; use those when testing actual blockchain inclusion or SDK broadcasting.

The reusable fixture in `assets/contract_test_fixture.rs` creates a real Elements transaction environment, instantiates a supplied `.simf` source with public parameters, signs its actual Simplicity transaction digest with deterministic test-only keys, resolves witnesses, and executes it. It contains no contract-specific policy or expected results. It supports signature, hash, multisig, and lock predicates; its empty scripts/synthetic outputs are not a full funded-chain transaction or a general covenant fixture. For covenant/output constraints, adapt the environment explicitly to the requested outputs and commitments. Create `tests/common/` with `mkdir -p tests/common`, then copy it to `tests/common/mod.rs`; do not edit the fixture to make a failing policy test pass. Write policy cases yourself in `tests/contract.rs`.

The initialized Cargo project already depends on `smplx-std`, whose Rust crate name is `simplex`. Add `hex = "0.4"` to `[dev-dependencies]` for the fixture. Pin `smplx-std = "=0.2.0"` for the verified toolchain. Dependencies must be pre-provisioned for offline Cargo. Use the supplied launcher/environment when the task provides one.

Minimal usage shape (replace argument/witness names and data with your contract's actual interface):

```rust
mod common;
use common::{evaluate, Tx};

const SOURCE: &str = include_str!("../simf/contract.simf");

#[test]
fn authorized_and_altered_transaction() {
    let parameters = [("OWNER", "@PK1")];
    let witness = [("AUTH", "@SIG1")];
    assert!(evaluate(SOURCE, &parameters, &witness, Tx::default()).is_ok());
    let error = evaluate(SOURCE, &parameters, &witness,
        Tx { mutate_output_after_signing: true, ..Tx::default() }).unwrap_err();
    assert_eq!(error.stage, "execution", "unexpected fixture/type failure: {error:?}");
}
```

`common::pubkey_hash(index)` returns a `0x`-prefixed SHA256 hash of the 32-byte x-only key for fixture indices 1 through 8. Before evaluating a hash-based contract, establish which fixture key matches the requested literal (compare computed hashes); use that key for the requested-hash positive test. For a negative hash case, first assert `assert_ne!(negative_hash, pubkey_hash(revealed_key_index))`, then evaluate the spend and require an execution failure. If no known fixture key matches a supplied hash, report that limitation instead of inventing its preimage. Compute this when testing a requested hash: never assume a deterministic fixture key differs from a supplied literal. A negative hash test must use a verified different value. Keep small parameter arrays inline to avoid unnecessary Rust lifetime helpers.

`@PK1` through `@PK8` are x-only public keys derived from deterministic secrets 1 through 8, for tests only. `@SIG1` through `@SIG8` sign the fixture transaction digest with those keys; `@INVALID` is a zeroed 64-byte signature. These strings can appear inside `Some(...)`, tuples, arrays, `Left(...)`, or `Right(...)`. The source's declared types parse the complete values; no witness/parameter ABI is guessed by the fixture.

`Tx` fields:

- `locktime: u32`, default 0.
- `sequence: u32`, default `0xfffffffe`, non-final so absolute locktime can be active.
- `version: u32`, default 2.
- `other_sequence: Option<u32>`, default None; adds another input for current-input-lock tests.
- `mutate_output_after_signing: bool`, default false; signs before changing an output's value, testing the transaction commitment.

`evaluate` returns `Ok(())` on successful program execution, or `Failure { stage, message }`. Stages are `compilation`, `arguments`, `witness-parsing`, `satisfaction`, and `execution`. For invalid signatures or a rejected policy gate, assert an execution failure, rather than accepting a misspelled witness or malformed test fixture as the desired rejection. Missing witnesses may fail at satisfaction; malformed witness encodings fail at witness parsing. Check those separately if they are relevant.

Write at least one positive test for each authorized path and negative tests for the actual restrictions. For optional multisig, enumerate all signature-presence subsets and invalidate a present signature in turn, including an extra invalid signature. For a lock, cover below/at/above the encoded limit, final/disabled/time flags, old transaction version, and another input's delay. Execution tests inspect transaction fields; they do not establish that a node will include a transaction at a particular block height.

Remove the initializer's `todo!()` dummy test. A passing suite must actually execute the spending predicate with realistic signatures; assertions only about strings, arithmetic, or a separate reimplementation of the policy do not test the contract.
