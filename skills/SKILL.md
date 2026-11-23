---
name: simplicityhl-simplex
description: Write, debug, and audit SimplicityHL contracts and Simplex projects for Liquid, including signatures, witnesses, timelocks, covenants, oracle messages, and asset conservation.
---

# SimplicityHL with Simplex

Write `.simf` spending conditions for Liquid using Simplex. SimplicityHL resembles Rust but has different syntax, types, and consensus semantics. Treat a contract as a predicate on a spending transaction: successful completion authorizes the spend; failed assertions reject it.

Run `simplex skill path` to locate this complete skill package. Resolve `references/` and `assets/` relative to the returned directory, not the project directory.

Use the user's supplied resources and access restrictions. These references and the local test fixture support development without external research. Use the CLI, compiler, tests, and your own project files when they are available. A restriction on external information does not prohibit these development tools. Do not fetch the source links during a closed-book task.

For an audit, read [audit.md](references/audit.md). Start from the supplied intended behavior and existing source. Preserve the audited source while constructing counterexamples; do not replace an audit with implementing a different policy. Read language, timelock, or testing references only as needed for the relevant checks.

## Workflow

1. Initialize a real project with `simplex init NAME` when the provided environment supports it. Inspect the generated `Cargo.toml`, `src/lib.rs`, and tests together. Edit the existing scaffolding: the initializer already declares `pub mod artifacts;`, so do not append it again. Replace the generated dummy test and sample contract as needed. Identify deployment parameters (`param::NAME`), spend-time witnesses (`witness::NAME`), and the authorization conditions of each branch. Deployment keys must not come from an unbound witness. Preserve exact numeric thresholds and literal hashes.
2. Check that the requested behavior is expressible. The contract cannot read current chain height or UTXO age. Timelocks enforce transaction lower bounds via consensus, not an upper deadline. Explain any mismatch explicitly and label a closest implementation as an approximation.
3. Write a self-contained contract using jets for small predicates. Read [language.md](references/language.md) for syntax and signature/hash patterns. Read [timelocks.md](references/timelocks.md) for any height/delay logic. Read [simplex.md](references/simplex.md) for project files, imports, generated bindings, and testing.
4. Build with `simplex build` from the project directory. Fix diagnostics from the actual pinned compiler. Built-in aliases such as `Pubkey` and `Signature` already exist; do not redeclare them. Confirm the intended contract appears in generated metadata: source discovery warns about unparseable files. Read the artifact-coverage check and bundled compiler-checker fallback in [simplex.md](references/simplex.md).
5. Write and run acceptance/rejection tests. Establish fixture data before labeling a case positive or negative: for a supplied public-key hash, compute `common::pubkey_hash(index)` and compare it to that literal. Never classify the literal as a mismatch by guessing. A hash-mismatch test must explicitly assert that its chosen hash differs from the revealed key's computed hash. For offline execution tests, use [testing.md](references/testing.md) and the reusable [execution fixture](assets/contract_test_fixture.rs): copy it into your project and write your own behavior cases. Run them through `simplex test --no-simplex --show-output`. Confirm actual tests ran; a plan or an empty passing suite is not validation. Also run `cargo check --offline` to verify the generated Rust bindings. Fix failures yourself, then rerun the affected checks. Distinguish compilation, execution in a transaction environment, and blockchain consensus validation.

Run build/check/test commands sequentially within a project. When wrapping a shell tool, display its `session_id`/`exit_code` as well as output; displaying only `result.output` hides the handle needed to wait. If a command returns a running session or cell ID, wait or poll that same ID until completion; do not launch the same command again. A second Cargo invocation waits on the first build lock and obscures the result. Batch independent file reads and inspect only references relevant to the request; copy the bundled fixture without rereading it in small chunks.

When the environment permits only the Simplex CLI, use `simplex test --no-simplex --show-output` to compile and check the Rust tests/bindings. Omit direct Cargo, Python, `simc`, and HAL commands. For skipped source diagnostics, use the compiler-probe Rust test in [simplex.md](references/simplex.md), run through `simplex test`; the standalone Python checker is for environments that permit it. Use workspace file operations to copy the fixture and create directories when shell utilities are unavailable.

Keep the implementation small. Prefer fixed tuple/array destructuring and small functions over speculative abstractions. Define callees before callers. Parameterize keys when the user has not supplied values. Do not invent live keys or private keys.

## Authorization

- `jet::bip_0340_verify((pk, jet::sig_all_hash()), sig)` verifies a BIP340 Schnorr signature over the complete Simplicity transaction digest. It returns `()`, failing on an invalid signature. Do not use it as a boolean.
- A `Pubkey` is the 32-byte x-only key (`u256`); `Signature` is a 64-byte value (`[u8; 64]`). Hash a revealed key's 32 bytes, not a compressed 33-byte encoding.
- If the user asks for exactly N signatures, every present signature must verify and the count must equal N. An invalid present signature must reject, even if N other signatures verify. Do not replace equality with a minimum threshold.
- Use `Option<Signature>` for optional signatures and `Either<...>` to encode mutually exclusive spend paths. An explicit `match` is evaluated only on the chosen branch.

## Covenants and state

Transaction outputs can be constrained using jets such as `num_outputs`, `output_script_hash`, and `output_amount`; unwrap optional output values and compare at the correct bit width. Matching only a script hash does not preserve asset or amount. State is carried by UTXOs; changing it requires spending and creating an output with an enforced new commitment. Avoid adding output restrictions when the user requests only a signature gate.

Sources and pinned versions: [provenance.md](references/provenance.md). This bundle targets Simplex 0.2.0 with SimplicityHL 0.8.0. Its files are shipped with the CLI version that exposes them.
