// Narrated Simplex demo for `last_will.simf`, a recursive covenant.
//
// These tests exercise various failure conditions in addition to the
// happy paths, showing how the contract logic and network consensus
// enforce spending rules.

use last_will_example::artifacts::last_will::LastWillProgram;
use last_will_example::artifacts::last_will::derived_last_will::{LastWillArguments, LastWillWitness};

use simplex::program::{ProgramTrait, WitnessTrait};
use simplex::simplicityhl::elements::pset::PartiallySignedTransaction;
use simplex::simplicityhl::elements::{Script, Sequence};
use bitcoin_hashes::Hash as _;
use simplex::simplicityhl::simplicity::bitcoin::secp256k1::{self, Keypair, Message, SecretKey};
use simplex::either::Either;
use simplex::simplicityhl::WitnessValues;
use simplex::provider::SimplicityNetwork;
use simplex::transaction::{FinalTransaction, PartialInput, PartialOutput, ProgramInput, RequiredSignature};

// The contract hardcodes its own public keys in source (`1*G`, `2*G`,
// `3*G`, for inherit/cold/hot, respectively) rather than taking them as
// `param::` arguments. The matching private keys are these same small
// integers. These hard-coded keys are just for demonstration purposes,
// while real deployments would use `param::` values for the real public
// keys.
fn scalar(n: u8) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[31] = n;
    bytes
}
const INHERITOR_KEY: u8 = 1;
const COLD_KEY: u8 = 2;
const HOT_KEY: u8 = 3;

// The original contract uses 25920 blocks, about 18 days at Liquid's
// 1-minute block time. This demo uses the smaller value 1440 (1 day)
// instead, purely for resource purposes. Mining tens of thousands of
// blocks on regtest may risk a `nextest` timeout.
const INHERITANCE_DISTANCE: u16 = 1440;

#[derive(Clone)]
struct SpendPathWitness {
    variant: &'static str,
    signature: [u8; 64],
}

impl SpendPathWitness {
    fn placeholder(_program: &LastWillProgram, variant: &'static str) -> anyhow::Result<Self> {
        Ok(Self {
            variant,
            signature: [0u8; 64],
        })
    }

    fn with_signature(&self, signature: [u8; 64]) -> Self {
        Self {
            variant: self.variant,
            signature,
        }
    }
}

impl WitnessTrait for SpendPathWitness {
    fn build_witness(&self) -> WitnessValues {
        // `witness::INHERIT_OR_NOT` is an `Either<Signature,
        // Either<Signature, Signature>>`, so each named spending path
        // is a branch of that type. Simplex's codegen generates a
        // real `LastWillWitness` for this shape, so all we have to do
        // is name the branch; the conversion into Simplicity values
        // is generated. Using the experimental `enum` feature would
        // produce more readable SimplicityHL code, but is currently not
        // supported in Simplex.
        let inherit_or_not = match self.variant {
            "Inherit" => Either::Left(self.signature),
            "ColdSpend" => Either::Right(Either::Left(self.signature)),
            "HotSpend" => Either::Right(Either::Right(self.signature)),
            other => panic!("unknown spending path: {other}"),
        };

        LastWillWitness { inherit_or_not }.build_witness()
    }
}

fn program() -> LastWillProgram {
    LastWillProgram::new(&LastWillArguments {})
}

fn fee_output(amount: u64, network: &SimplicityNetwork) -> PartialOutput {
    // Unlike Bitcoin, Elements/Liquid represents the transaction
    // fee as an explicit output with an empty scriptPubKey.
    // `recursive_covenant()` specifically checks for this at output
    // index 1, using `jet::output_is_fee(1)`.
    PartialOutput::new(Script::new(), amount, network.policy_asset())
}

/// The destination doesn't matter for demo purposes, so it just goes
/// back to the test wallet's own address.
fn remainder_output(context: &simplex::TestContext, amount: u64) -> PartialOutput {
    PartialOutput::new(
        context.get_default_signer().get_address().script_pubkey(),
        amount,
        context.get_network().policy_asset(),
    )
}

/// `mine_until_height` issues a single RPC call asking the node to
/// generate however many blocks are needed in one shot. Asking for too
/// many blocks in one call can cause timeout problems, so we mine in
/// smaller batches here.
fn mine_up_to(context: &simplex::TestContext, target: u64) -> anyhow::Result<()> {
    const BATCH: u64 = 500;
    loop {
        let current = u64::from(context.get_default_provider().fetch_tip_height()?);
        if current >= target {
            return Ok(());
        }
        let next = (current + BATCH).min(target);
        context.get_network_utils().mine_until_height(next)?;
        println!("  ...mined to height {next} ({} to go)", target - next);
    }
}

/// Signs `pst`'s input at `input_index` with a BIP-340 signature
/// over `jet::sig_all_hash()`, using the given raw 32-byte private
/// key scalar directly. Here, the possible private keys are
/// hard-coded.
fn sign_with_key(
    pst: &PartiallySignedTransaction,
    program: &LastWillProgram,
    input_index: usize,
    network: &SimplicityNetwork,
    raw_privkey: [u8; 32],
) -> [u8; 64] {
    let env = program
        .as_ref()
        .get_env(pst, input_index, network)
        .expect("env should build");
    let msg = Message::from_digest(env.c_tx_env().sighash_all().to_byte_array());

    let secp = secp256k1::Secp256k1::new();
    let secret_key = SecretKey::from_slice(&raw_privkey).expect("valid scalar");
    let keypair = Keypair::from_secret_key(&secp, &secret_key);

    secp.sign_schnorr(&msg, &keypair).serialize()
}

/// Funds a fresh covenant instance, builds a `FinalTransaction`
/// spending it with the given `variant`/`sequence`/`outputs`, signs it
/// with `signing_key`, finalizes locally, and broadcasts. Returns the
/// local-execution or broadcast error (if any) unexamined, so callers
/// can assert either success or a specific failure result.
///
/// `description` is printed immediately before the actual submission,
/// and (only on success) immediately after. The `nextest` wrapper will
/// capture the output and print it only when the test completes.
fn spend_action(
    context: &simplex::TestContext,
    description: &str,
    variant: &'static str,
    sequence: Sequence,
    signing_key: [u8; 32],
    outputs: Vec<PartialOutput>,
    utxo_amount: u64,
) -> anyhow::Result<String> {
    let prog = program();
    let script = prog.get_script_pubkey(context.get_network());

    context.get_default_signer().send(script.clone(), utxo_amount)?;

    let utxos = context.get_default_provider().fetch_scripthash_utxos(&script)?;
    let placeholder = SpendPathWitness::placeholder(&prog, variant)?;

    let mut ft = FinalTransaction::new();
    ft.add_program_input(
        PartialInput::new(utxos[0].clone()).with_sequence(sequence),
        ProgramInput::new(Box::new(prog.as_ref().clone()), Box::new(placeholder.clone())),
        RequiredSignature::None,
    );
    for output in outputs {
        ft.add_output(output);
    }

    let (mut pst, _secrets) = ft.extract_pst();

    let sig = sign_with_key(&pst, &prog, 0, context.get_network(), signing_key);
    let real_witness = placeholder.with_signature(sig);

    println!("Submitting {description}...");

    // Local Simplicity execution: this is where an
    // `AssertFailed`/`PrunedBranch` case would surface, before the
    // transaction ever reaches the network. A failure here (or at
    // broadcast, below) propagates via `?` without printing a "->
    // rejected" line itself -- that's the caller's job, since callers
    // differ on how they want to describe an expected rejection (a
    // specific error-message check, a generic assertion-failure check,
    // and so on).
    let pruned_witness = prog
        .as_ref()
        .finalize(&pst, &real_witness.build_witness(), 0, context.get_network())?;
    pst.inputs_mut()[0].final_script_witness = Some(pruned_witness);

    let tx = pst.extract_tx()?;
    let receipt = context.get_default_provider().broadcast_transaction(&tx)?;
    println!("  -> accepted: {receipt}");
    Ok(receipt.to_string())
}

fn seq_distance(blocks: u16) -> Sequence {
    Sequence::from_consensus(u32::from(blocks))
}

/// Asserts that `spend_action`'s result failed during LOCAL Simplicity
/// execution (`AssertFailed`) rather than succeeding or failing only
/// once it reached a node. Every test that expects a spend to be
/// rejected outright (wrong key, or a `recursive_covenant()` violation)
/// fails this way: the contract's own logic catches it before broadcast
/// is ever attempted.
fn expect_assert_failure(result: anyhow::Result<String>, what_was_wrong: &str) -> anyhow::Result<()> {
    match result {
        Ok(receipt) => anyhow::bail!("expected local execution to reject this ({what_was_wrong}), but it broadcast: {receipt}"),
        Err(err) => {
            let msg = err.to_string();
            anyhow::ensure!(
                msg.contains("Jet failed during execution"),
                "expected an assertion failure ({what_was_wrong}), got a different error: {msg}"
            );
            println!("  -> rejected locally, as expected: {what_was_wrong}\n");
            Ok(())
        }
    }
}

/// A simple non-covenant script (OP_TRUE), as an example of a script
/// that definitely *is not* equal to this covenant (to demonstrate
/// the covenant's ability to distinguish copies of itself from other
/// scripts).
fn unrelated_script() -> Script {
    Script::from(vec![0x51])
}

#[simplex::test]
fn cold_spend_breaks_out_of_the_covenant(context: simplex::TestContext) -> anyhow::Result<()> {
    println!(
        "=== ColdSpend: the owner cancels the covenant entirely ===\n\n\
         `cold_spend` is the simplest of the three paths -- just a signature check\n\
         against the hardcoded cold key (2*G). Whoever holds the cold private key\n\
         can withdraw the funds from this covenant at any time, ending it.\n"
    );

    spend_action(
        &context,
        "ColdSpend transaction, signed with the cold key",
        "ColdSpend",
        Sequence::default(),
        scalar(COLD_KEY),
        vec![
            remainder_output(&context, 99_000),
            fee_output(1_000, context.get_network()),
        ],
        100_000,
    )?;
    Ok(())
}

#[simplex::test]
fn hot_spend_refreshes_the_covenant(context: simplex::TestContext) -> anyhow::Result<()> {
    println!(
        "=== HotSpend: the owner checks in, and the covenant is refreshed ===\n\n\
         This is the core covenant example: `refresh_spend` checks a signature but also\n\
         calls `recursive_covenant()`, which requires the transaction to have exactly\n\
         two outputs: output 0 must be a NEW UTXO paying back to this exact same\n\
         covenant script (verified by comparing commitment hashes, not by trusting the\n\
         witness), and output 1 must be a fee output. This is how a Simplicity covenant\n\
         'lives on' across transactions, which Bitcoin Script can't express because it\n\
         has no way to inspect or constrain transaction outputs.\n"
    );

    let prog = program();
    let script = prog.get_script_pubkey(context.get_network());

    spend_action(
        &context,
        "HotSpend transaction, signed with the hot key",
        "HotSpend",
        Sequence::default(),
        scalar(HOT_KEY),
        vec![
            PartialOutput::new(script.clone(), 99_000, context.get_network().policy_asset()),
            fee_output(1_000, context.get_network()),
        ],
        100_000,
    )?;

    println!(
        "The covenant's {INHERITANCE_DISTANCE}-block inheritance clock has effectively\n\
         been reset -- the new UTXO this transaction creates is a fresh instance of the\n\
         same covenant, with the same overall rules as the original.\n"
    );
    Ok(())
}

#[simplex::test]
fn inherit_too_early_is_rejected(context: simplex::TestContext) -> anyhow::Result<()> {
    println!(
        "=== Inherit, attempted immediately: rejected by the network ===\n\n\
         The inheritor declares a sequence number claiming the full {INHERITANCE_DISTANCE}-block\n\
         wait has passed. `enforce_relative_distance` only checks that the DECLARED\n\
         value is large enough -- it can't see the real chain state, so local execution\n\
         succeeds. But we deliberately don't mine any blocks first, so the real distance\n\
         elapsed since funding falls short of the declared value. The node itself then\n\
         catches this and rejects the transaction as too early.\n"
    );

    let result = spend_action(
        &context,
        &format!(
            "Inherit transaction, declaring the full {INHERITANCE_DISTANCE}-block wait, with no blocks actually mined"
        ),
        "Inherit",
        seq_distance(INHERITANCE_DISTANCE),
        scalar(INHERITOR_KEY),
        vec![
            remainder_output(&context, 99_000),
            fee_output(1_000, context.get_network()),
        ],
        100_000,
    );

    match result {
        Ok(receipt) => {
            anyhow::bail!("expected the node to reject this broadcast, but it succeeded: {receipt}")
        }
        Err(err) => {
            let msg = err.to_string();
            anyhow::ensure!(
                msg.contains("non-BIP68-final"),
                "expected a `non-BIP68-final` rejection specifically, got: {msg}"
            );
            println!("  -> rejected by the node: {msg}\n");
            println!(
                "The `non-BIP68-final` message is triggered by the node's mempool policy,\n\
                 showing how the timelock guarantee is enforced by the network.\n"
            );
        }
    }
    Ok(())
}

#[simplex::test]
fn inherit_after_wait_succeeds(context: simplex::TestContext) -> anyhow::Result<()> {
    println!(
        "=== Inherit successfully: fast-forwarding {INHERITANCE_DISTANCE} blocks ===\n\n\
         At Liquid's 1-minute block time, {INHERITANCE_DISTANCE} blocks is about a day,\n\
         against the 25920 blocks (~18 days) the upstream contract asks for. In a\n\
         real deployment, the inheritor has to wait the correct amount of time\n\
         before inheriting. For demonstration purposes, we can fast-forward the\n\
         network by asking the regtest to mine the required number of blocks\n\
         immediately, ignoring its default blocktime delay.\n"
    );

    let prog = program();
    let script = prog.get_script_pubkey(context.get_network());
    context.get_default_signer().send(script.clone(), 100_000)?;

    let target = context.get_default_provider().fetch_tip_height()? as u64 + u64::from(INHERITANCE_DISTANCE);
    mine_up_to(&context, target)?;
    println!("Mined up to height {target}.\n");

    let utxos = context.get_default_provider().fetch_scripthash_utxos(&script)?;
    let placeholder = SpendPathWitness::placeholder(&prog, "Inherit")?;

    let mut ft = FinalTransaction::new();
    ft.add_program_input(
        PartialInput::new(utxos[0].clone()).with_sequence(seq_distance(INHERITANCE_DISTANCE)),
        ProgramInput::new(Box::new(prog.as_ref().clone()), Box::new(placeholder.clone())),
        RequiredSignature::None,
    );
    ft.add_output(remainder_output(&context, 99_000));
    ft.add_output(fee_output(1_000, context.get_network()));

    let (mut pst, _secrets) = ft.extract_pst();
    let sig = sign_with_key(&pst, &prog, 0, context.get_network(), scalar(INHERITOR_KEY));
    let real_witness = placeholder.with_signature(sig);

    let pruned_witness = prog
        .as_ref()
        .finalize(&pst, &real_witness.build_witness(), 0, context.get_network())?;
    pst.inputs_mut()[0].final_script_witness = Some(pruned_witness);

    println!("Submitting the same Inherit transaction as before, now that the wait is real...");
    let tx = pst.extract_tx()?;
    let receipt = context.get_default_provider().broadcast_transaction(&tx)?;
    println!("  -> accepted: {receipt}");

    println!(
        "\nThe same spend that was rejected before now succeeds. The contract and\n\
         witness are the same, but the blockchain state has changed.\n"
    );
    Ok(())
}

// =====================================================================
// Wrong-key tests: each of the three paths checks a signature against a
// specific hardcoded public key. These tests deliberately mix up these
// keys and so fail during local execution.
// =====================================================================

#[simplex::test]
fn hot_spend_with_cold_key_fails(context: simplex::TestContext) -> anyhow::Result<()> {
    println!(
        "=== HotSpend, signed with the cold key instead of the hot key ===\n\n\
         Correctly structured HotSpend, but signed with the cold private key (2)\n\
         instead of the expected hot key (3). The signature will be rejected.\n"
    );
    let prog = program();
    let script = prog.get_script_pubkey(context.get_network());
    let result = spend_action(
        &context,
        "HotSpend transaction, signed with the cold key instead of the hot key",
        "HotSpend",
        Sequence::default(),
        scalar(COLD_KEY),
        vec![
            PartialOutput::new(script.clone(), 99_000, context.get_network().policy_asset()),
            fee_output(1_000, context.get_network()),
        ],
        100_000,
    );
    expect_assert_failure(result, "cold key signed a HotSpend")
}

#[simplex::test]
fn hot_spend_with_inheritor_key_fails(context: simplex::TestContext) -> anyhow::Result<()> {
    println!(
        "=== HotSpend, signed with the inheritor's key instead of the hot key ===\n\n\
         Same idea as the cold-key case, with the third key instead: the inheritor\n\
         cannot use their key to act as the owner and bypass the timelock.\n"
    );
    let prog = program();
    let script = prog.get_script_pubkey(context.get_network());
    let result = spend_action(
        &context,
        "HotSpend transaction, signed with the inheritor's key instead of the hot key",
        "HotSpend",
        Sequence::default(),
        scalar(INHERITOR_KEY),
        vec![
            PartialOutput::new(script.clone(), 99_000, context.get_network().policy_asset()),
            fee_output(1_000, context.get_network()),
        ],
        100_000,
    );
    expect_assert_failure(result, "inheritor key signed a HotSpend")
}

#[simplex::test]
fn cold_spend_with_hot_key_fails(context: simplex::TestContext) -> anyhow::Result<()> {
    println!(
        "=== ColdSpend, signed with the hot key instead of the cold key ===\n\n\
         Even though the two owner-held keys (hot and cold) belong to the same person\n\
         in practice, they aren't interchangeable and have different abilities.\n\
         Here we demonstrate this by trying to sign a ColdSpend with the hot key.\n"
    );
    let result = spend_action(
        &context,
        "ColdSpend transaction, signed with the hot key instead of the cold key",
        "ColdSpend",
        Sequence::default(),
        scalar(HOT_KEY),
        vec![
            remainder_output(&context, 99_000),
            fee_output(1_000, context.get_network()),
        ],
        100_000,
    );
    expect_assert_failure(result, "hot key signed a ColdSpend")
}

#[simplex::test]
fn cold_spend_with_inheritor_key_fails(context: simplex::TestContext) -> anyhow::Result<()> {
    println!("=== ColdSpend, signed with the inheritor's key ===\n\nSame principle, third key.\n");
    let result = spend_action(
        &context,
        "ColdSpend transaction, signed with the inheritor's key",
        "ColdSpend",
        Sequence::default(),
        scalar(INHERITOR_KEY),
        vec![
            remainder_output(&context, 99_000),
            fee_output(1_000, context.get_network()),
        ],
        100_000,
    );
    expect_assert_failure(result, "inheritor key signed a ColdSpend")
}

#[simplex::test]
fn inherit_with_hot_key_after_wait_fails(context: simplex::TestContext) -> anyhow::Result<()> {
    println!(
        "=== Inherit, signed with the hot key instead of the inheritor's key ===\n\n\
         Once again, the keys are not interchangeable and the key used must match the\n\
         specific spending path it is supposed to authorize.\n"
    );
    let result = spend_action(
        &context,
        "Inherit transaction (declared wait satisfied), signed with the hot key",
        "Inherit",
        seq_distance(INHERITANCE_DISTANCE),
        scalar(HOT_KEY),
        vec![
            remainder_output(&context, 99_000),
            fee_output(1_000, context.get_network()),
        ],
        100_000,
    );
    expect_assert_failure(result, "hot key signed an Inherit")
}

#[simplex::test]
fn inherit_with_cold_key_after_wait_fails(context: simplex::TestContext) -> anyhow::Result<()> {
    println!("=== Inherit, signed with the cold key ===\n\nSame principle, the other owner-held key.\n");
    let result = spend_action(
        &context,
        "Inherit transaction (declared wait satisfied), signed with the cold key",
        "Inherit",
        seq_distance(INHERITANCE_DISTANCE),
        scalar(COLD_KEY),
        vec![
            remainder_output(&context, 99_000),
            fee_output(1_000, context.get_network()),
        ],
        100_000,
    );
    expect_assert_failure(result, "cold key signed an Inherit")
}

// =====================================================================
// `recursive_covenant()` violations. Each condition enforcing the
// structure of the covenant is broken on purpose by one of these tests.
// =====================================================================

#[simplex::test]
fn hot_spend_wrong_output_count_fails(context: simplex::TestContext) -> anyhow::Result<()> {
    println!(
        "=== HotSpend with only one output ===\n\n\
         `recursive_covenant()`'s first check is that `num_outputs()` equals 2.\n\
         Here, the entire 100,000 sats is provided as a single fee output.\n\
         Since this contract does not constrain the *value* of the fee output, a\n\
         stolen hot key could still leak most of the balance to miners this way.\n\
         This test only shows that skipping the continuation output entirely\n\
         is caught.\n"
    );
    let result = spend_action(
        &context,
        "HotSpend transaction with a single (fee-only) output",
        "HotSpend",
        Sequence::default(),
        scalar(HOT_KEY),
        vec![fee_output(100_000, context.get_network())],
        100_000,
    );
    expect_assert_failure(result, "HotSpend with 1 output instead of 2")
}

#[simplex::test]
fn hot_spend_wrong_continuation_script_fails(context: simplex::TestContext) -> anyhow::Result<()> {
    println!(
        "=== HotSpend recreating the wrong script ===\n\n\
         Here, output 0 sends the money to a script output different from this exact\n\
         covenant. `recursive_covenant()` compares the output's script hash against this\n\
         program's own script hash directly (not by trusting the witness), so a HotSpend\n\
         can't be used to smuggle the funds anywhere else while disguised as a routine\n\
         refresh.\n"
    );
    let result = spend_action(
        &context,
        "HotSpend transaction recreating an unrelated script, not this covenant",
        "HotSpend",
        Sequence::default(),
        scalar(HOT_KEY),
        vec![
            PartialOutput::new(unrelated_script(), 99_000, context.get_network().policy_asset()),
            fee_output(1_000, context.get_network()),
        ],
        100_000,
    );
    expect_assert_failure(result, "HotSpend output 0 recreates the wrong script")
}

#[simplex::test]
fn hot_spend_non_fee_second_output_fails(context: simplex::TestContext) -> anyhow::Result<()> {
    println!(
        "=== HotSpend with a real script where the fee output should be ===\n\n\
         Output 0 correctly recreates the covenant. Output 1, though, has a real\n\
         destination script instead of the empty scriptPubKey that marks an actual fee\n\
         output -- so `jet::output_is_fee(1)` returns `Some(false)`, `unwrap` yields\n\
         `false`, and the assertion fails on an ordinary `false`.\n"
    );
    let prog = program();
    let script = prog.get_script_pubkey(context.get_network());
    let result = spend_action(
        &context,
        "HotSpend transaction with a non-fee second output",
        "HotSpend",
        Sequence::default(),
        scalar(HOT_KEY),
        vec![
            PartialOutput::new(script.clone(), 99_000, context.get_network().policy_asset()),
            PartialOutput::new(unrelated_script(), 1_000, context.get_network().policy_asset()),
        ],
        100_000,
    );
    expect_assert_failure(result, "HotSpend output 1 isn't a real fee output")
}

#[simplex::test]
fn hot_spend_swapped_outputs_fails(context: simplex::TestContext) -> anyhow::Result<()> {
    println!(
        "=== HotSpend with the fee and continuation outputs swapped ===\n\n\
         The order of outputs can be meaningful and can be enforced with SimplicityHL.\n\
         Here, a real continuation output and a real fee output are present, but at the\n\
         wrong indices. Both of `recursive_covenant()`'s checks are position-specific\n\
         (`output_script_hash(0)`, `output_is_fee(1)`), not \"does some output satisfy\n\
         this somewhere,\" so swapping them fails.\n"
    );
    let prog = program();
    let script = prog.get_script_pubkey(context.get_network());
    let result = spend_action(
        &context,
        "HotSpend transaction with fee/continuation outputs swapped",
        "HotSpend",
        Sequence::default(),
        scalar(HOT_KEY),
        vec![
            fee_output(1_000, context.get_network()),
            PartialOutput::new(script.clone(), 99_000, context.get_network().policy_asset()),
        ],
        100_000,
    );
    expect_assert_failure(result, "HotSpend outputs in the wrong order")
}

// ========================================================================
// A realistic multi-hop spending chain: the covenant is refreshed twice by
// the owner and then gets broken out of with ColdSpend.
// ========================================================================

#[simplex::test]
fn covenant_survives_two_refreshes_then_breaks_out(context: simplex::TestContext) -> anyhow::Result<()> {
    println!(
        "=== A full lifecycle: refresh, refresh again, then break out ===\n\n\
         Three chained transactions: the owner checks in twice with the hot key --\n\
         each one spending the UTXO the *previous* transaction created, proving the\n\
         covenant persists across several hops rather than just surviving a single\n\
         refresh, then the owner finally exits via the cold key. The covenant script\n\
         itself is the same at every step.\n"
    );

    let prog = program();
    let script = prog.get_script_pubkey(context.get_network());
    let network = context.get_network();
    let asset = network.policy_asset();

    context.get_default_signer().send(script.clone(), 100_000)?;

    let hot_spend_hop = |amount_in: u64, hop_name: &str| -> anyhow::Result<()> {
        let utxos = context.get_default_provider().fetch_scripthash_utxos(&script)?;
        let placeholder = SpendPathWitness::placeholder(&prog, "HotSpend")?;

        let mut ft = FinalTransaction::new();
        ft.add_program_input(
            PartialInput::new(utxos[0].clone()),
            ProgramInput::new(Box::new(prog.as_ref().clone()), Box::new(placeholder.clone())),
            RequiredSignature::None,
        );
        ft.add_output(PartialOutput::new(script.clone(), amount_in - 1_000, asset));
        ft.add_output(fee_output(1_000, network));

        let (mut pst, _secrets) = ft.extract_pst();
        let sig = sign_with_key(&pst, &prog, 0, network, scalar(HOT_KEY));
        let real_witness = placeholder.with_signature(sig);

        let pruned_witness = prog.as_ref().finalize(&pst, &real_witness.build_witness(), 0, network)?;
        pst.inputs_mut()[0].final_script_witness = Some(pruned_witness);

        println!("Submitting {hop_name} (HotSpend, {amount_in} sats in, {} sats continuing)...", amount_in - 1_000);
        let tx = pst.extract_tx()?;
        let receipt = context.get_default_provider().broadcast_transaction(&tx)?;
        receipt.wait()?;
        println!("  -> accepted and confirmed: {receipt}\n");
        Ok(())
    };

    hot_spend_hop(100_000, "first refresh")?;
    println!("Covenant still alive at 99,000 sats.\n");
    hot_spend_hop(99_000, "second refresh")?;
    println!("Covenant still alive at 98,000 sats.\n");

    // Final hop: break out via ColdSpend, spending the real UTXO the second refresh
    // created.
    let utxos = context.get_default_provider().fetch_scripthash_utxos(&script)?;
    let placeholder = SpendPathWitness::placeholder(&prog, "ColdSpend")?;
    let mut ft = FinalTransaction::new();
    ft.add_program_input(
        PartialInput::new(utxos[0].clone()),
        ProgramInput::new(Box::new(prog.as_ref().clone()), Box::new(placeholder.clone())),
        RequiredSignature::None,
    );
    ft.add_output(remainder_output(&context, 97_000));
    ft.add_output(fee_output(1_000, network));

    let (mut pst, _secrets) = ft.extract_pst();
    let sig = sign_with_key(&pst, &prog, 0, network, scalar(COLD_KEY));
    let real_witness = placeholder.with_signature(sig);
    let pruned_witness = prog.as_ref().finalize(&pst, &real_witness.build_witness(), 0, network)?;
    pst.inputs_mut()[0].final_script_witness = Some(pruned_witness);

    println!("Submitting final hop (ColdSpend, breaking out of the covenant for good)...");
    let tx = pst.extract_tx()?;
    let receipt = context.get_default_provider().broadcast_transaction(&tx)?;
    println!("  -> accepted: {receipt}\n");

    println!(
        "The covenant was recreated through two refresh cycles and then\n\
         deliberately ended. This is a somewhat realistic example of a\n\
         full lifecycle of an on-chain last-will covenant.\n"
    );
    Ok(())
}
