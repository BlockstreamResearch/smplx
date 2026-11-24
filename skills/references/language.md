# Syntax and common predicates

## Language boundary

- Entry point: `fn main() { ... }` with no arguments and a unit result.
- All `let` bindings require an explicit type after the entire pattern. For destructuring, use `let (carry, total): (bool, u8) = jet::add_8(a, b);`. Do not put types inside a tuple `let` pattern (`let (carry: bool, total: u8) = ...` is invalid). This differs from the inline types required in `match` payload bindings below.
- Fixed integers: `u1`, `u2`, `u4`, `u8`, `u16`, `u32`, `u64`, `u128`, `u256`; `bool` is distinct from `u1`.
- Number literals use bare decimal or hex values such as `0`, `2`, and `0xff`, with types supplied by the surrounding expression or binding. Rust suffixes such as `0u8` are invalid. Use `let zero: u8 = 0;` if a separately typed value is needed.
- Built-in aliases include `Pubkey = u256`, `Signature = [u8; 64]`, `Message = u256`, `Height = u32`, `Time = u32`, `Distance = u16`, `Duration = u16`, and `Ctx8` (SHA256 context). These names are already available: use them directly. **Do not declare `type Pubkey = ...`, `type Signature = ...`, or redefine another built-in alias.** The equals signs here explain the types; they are not declarations to copy into a `.simf` file. Redeclaration is a compiler error.
- Tuples `(A, B)`, arrays `[T; N]`, `Option<T>` (`None`, `Some(value)`), `Either<A, B>` (`Left(value)`, `Right(value)`). Destructure fixed arrays with `let [a, b, c]: [T; 3] = values;`.
- `match` payload bindings need types: `Some(sig: Signature) => ...`, `Left(sig: Signature) => ...`. For ordinary sum types, use two exhaustive arms. Nested sums need nested matches.
- No Rust arithmetic/comparison/boolean operators, `if`, `while`, `for`, indexing syntax, methods, early `return`, or recursive calls. Use jets and `match`. Bindings are immutable; shadowing is supported.
- Functions must be defined before they are called. A final expression without `;` is a return value; `assert!(...)` and signature verification return unit.
- `param::NAME` and `witness::NAME` are inferred from typed use sites. Put explicit type annotations on these bindings. Do not define `mod param` or `mod witness` inside the contract; concrete values are supplied separately at instantiation/spend time.
- `assert!(condition)` rejects when false. `panic!()` rejects unconditionally. `unwrap(option)` rejects `None`; `unwrap_left::<OtherType>(either)` rejects `Right`; `unwrap_right::<OtherType>(either)` rejects `Left`.

## Signature and SHA256 helpers

```simplicityhl
fn verify_transaction(pk: Pubkey, sig: Signature) {
    let digest: u256 = jet::sig_all_hash();
    jet::bip_0340_verify((pk, digest), sig);
}

fn sha256_32(bytes: u256) -> u256 {
    let ctx: Ctx8 = jet::sha_256_ctx_8_init();
    let ctx: Ctx8 = jet::sha_256_ctx_8_add_32(ctx, bytes);
    jet::sha_256_ctx_8_finalize(ctx)
}
```

For a hash gate, compute the hash then `assert!(jet::eq_256(computed, expected));`. SHA256 state initialization, add-32, and finalization implement SHA256 over exactly the 32-byte value. Equality on a `Pubkey`/hash uses `eq_256`, a block height `eq_32`, and a small signature count `eq_8`.

## Optional signatures and exact thresholds

Verify a present signature before counting it. Example helper:

```simplicityhl
fn optional_count(pk: Pubkey, maybe_sig: Option<Signature>) -> u8 {
    match maybe_sig {
        None => 0,
        Some(sig: Signature) => {
            verify_transaction(pk, sig);
            1
        },
    }
}
```

For a fixed small number of slots, unroll calls. `jet::add_8(a, b) -> (bool, u8)` returns `(carry, sum)`; assert carry is false with `match carry { false => (), true => panic!(), };` when overflow is possible. For sums of at most three 0/1 values, there cannot be a carry; document that bound if discarding it. Compare the final count using equality when the requirement says "exactly". Use a deployment `u8` parameter for a configurable threshold, optionally asserting it lies in the intended range.

## Useful jets

| Call | Result | Purpose |
| --- | --- | --- |
| `jet::eq_8(a, b)`, `eq_16`, `eq_32`, `eq_64`, `eq_256` | `bool` | Equality at the specified width |
| `jet::lt_32(a, b)`, `le_32`, `le_16`, `le_64` | `bool` | Less than / less than or equal |
| `jet::add_8(a, b)`, `add_32`, `add_64` | `(bool, uint)` | Carry and sum |
| `jet::subtract_64(a, b)` | `(bool, u64)` | Borrow and difference |
| `jet::sig_all_hash()` | `u256` | Complete transaction signature digest |
| `jet::bip_0340_verify((pk, msg), sig)` | `()` | Assert valid signature |
| `jet::current_sequence()` | `u32` | Current input's sequence |
| `jet::version()` | `u32` | Spending transaction version |
| `jet::tx_lock_height()` | `Height` | Effective transaction height lock; not chain height |
| `jet::num_outputs()` | `u32` | Output count |
| `jet::current_script_hash()` | `u256` | Hash of current input script |
| `jet::output_script_hash(index)` | `Option<u256>` | Hash of output script |
| `jet::output_is_fee(index)` | `Option<bool>` | Whether output is a fee output |

Values serialized for arguments/witnesses use language literals: hex numbers start `0x`; optional signatures use `None`/`Some(0x...)`; arrays use `[a, b, c]`; sum paths use `Left(...)`/`Right(...)`. JSON `.wit` entries can be bare literal strings resolved against declared types, or `{ "value": "...", "type": "..." }`. A concrete signature must have 64 bytes. Public parameters belong in an argument file, not a spender-controlled witness.
