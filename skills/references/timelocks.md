# Timelocks and their limits

The program sees transaction fields, not current blockchain height/time or the actual age of a UTXO. Consensus rejects transactions whose declared minimum time has not yet arrived. Checking a witness named "current height" is not a trustworthy time check.

## Absolute minimum block height

`jet::check_lock_height(min_height)` asserts the effective transaction lock height is at least `min_height`. Use `Height` (`u32`). The transaction must encode a height lock (`nLockTime < 500000000`) and have a non-final sequence, commonly `0xfffffffe`. Final sequences can disable effective locktime. Actual inclusion rules are separate from program execution; do not claim that executing a program proves a transaction can enter a block at an exact boundary.

## Relative minimum blocks: current input workaround

Do not use `check_lock_distance`, `check_lock_duration`, `tx_lock_distance`, or `tx_lock_duration`: these deprecated jets aggregate input locks incorrectly. Inspect the current input's sequence, require transaction version at least 2, and reject a disabled or time-based sequence for a block delay:

```simplicityhl
fn require_block_delay(minimum: Distance) {
    assert!(jet::le_32(2, jet::version()));
    let parsed: Option<Either<Distance, Duration>> =
        jet::parse_sequence(jet::current_sequence());
    match parsed {
        None => panic!(),
        Some(lock: Either<Distance, Duration>) => match lock {
            Left(blocks: Distance) => assert!(jet::le_16(minimum, blocks)),
            Right(seconds: Duration) => panic!(),
        },
    }
}
```

For a 1000-block delay, use current-input `nSequence = 1000` with transaction version at least 2. BIP68 disable bit (bit 31) and time-unit bit (bit 22) matter; `parse_sequence` rejects/identifies them. Another input with a large sequence must not satisfy this input's delay. `Duration` uses 512-second units, not seconds.

## Deadlines and refund branches

Timelocks only impose lower bounds. A cooperatively authorized spend valid before a timeout remains possible afterward until a competing refund spends the UTXO. Comparing `tx_lock_height()` to a maximum does not enforce an expiry: a spender can keep an old locktime.

When a user asks for cooperative approval before a deadline and a unilateral refund after it, a useful expressible design is an `Either` witness: cooperative signatures in one branch; the refund signature plus minimum lock in the other. State plainly that the cooperative branch remains usable after the timeout. If strict expiry is required, report that it cannot be implemented as a standalone predicate with the given inputs; discuss external preemption/state updates only as a separate design requiring additional assumptions.

An escrow permitting immediate two-party agreement and delayed sender refund should retain the immediate agreement branch. Apply the relative lock only in the refund branch; requiring it globally would prevent immediate agreements. The escrow key alone must never satisfy a branch. Exactly two signatures means reject a third present signature in the agreement branch.
