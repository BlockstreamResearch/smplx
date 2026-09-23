# Automatic witness padding

Prepare a new program before funding its address:

```rust
let program = signer.prepare_program(Program::new(source, &arguments))?;
let address = program.get_tr_address(&network);
```

Use this program in `ProgramInput` and finalize normally.
The framework supplies padding automatically.
Application witnesses need no padding field.

Generated bindings keep their typed helpers:

```rust
let program = MyProgram::new(&arguments).with_automatic_padding()?;
```

- Preparation changes the CMR and address, so it cannot retrofit an already funded program.
- The padding size covers the unpruned execution bound, including the padding prefix's cost.
- The signer checks the final witness budget and includes padding in fee estimation.
- `program::padding::MAX_PADDING_BYTES` limits padding to 256 KiB.
- This limits witness size, not application cost, because the padding itself consumes execution budget.
- The signer rejects transactions above 400,000 weight units.
- Retain the same source, arguments, SDK, compiler, dependency lockfile and build configuration when spending from the prepared address.

The tested SimplicityHL 0.7.2 and simplicity-lang 0.8.0 combination allows an application bound of at most 169,729.791 weight units with the largest prefix.
Preparation reports `PaddingLimit` if the complete bound cannot fit.
Golden tests pin each prefix's CMR and cost so dependency changes require an explicit compatibility review.

The signer and generated binding methods compile eagerly and return preparation errors.
`Program::with_automatic_padding()` is a lazy builder followed by `program.prepare()?` when using `Program` directly.
Call `prepare()` before address or CMR getters to receive errors instead of a panic.

When no change output is possible, `estimate_fee()` reports the entire remaining policy-asset amount that finalization pays as fee.
If the transaction has insufficient funds, it reports the estimated required fee instead.

Run the SDK tests:

```sh
cargo test -p smplx-sdk
```

With `elementsd` and `elements-cli` on PATH, run the local standard-policy test:

```sh
cargo test -p smplx-sdk --test automatic_padding_regtest -- --ignored
```
