// Narrated Simplex demo for `last_will.simf`, a recursive covenant.
//
// These tests exercise various failure conditions in addition to the
// happy paths, showing how the contract logic and network consensus
// enforce spending rules.

use last_will_example::artifacts::last_will::LastWillProgram;
use last_will_example::artifacts::last_will::derived_last_will::{LastWillArguments, LastWillWitness};

use simplex::constants::DUMMY_SIGNATURE;
use simplex::either::Either;
use simplex::provider::{ProviderError, SimplicityNetwork};
use simplex::signer::{Signer, SignerError};
use simplex::simplicityhl::elements::{Script, Sequence, Txid};
use simplex::transaction::{
    ChangeOutput, FinalTransaction, PartialInput, PartialOutput, ProgramInput, RequiredSignature, UTXO,
};

// The three roles each get their own wallet, built from its own BIP-39
// mnemonic. The contract takes their public keys as `param::` arguments, so
// the covenant is bound to these three wallets rather than to keys written
// into the contract source.
//
// These are standard BIP-39 test vectors. They are public knowledge and must
// never hold real funds.
const INHERITOR_MNEMONIC: &str =
    "legal winner thank year wave sausage worth useful legal winner thank yellow";
const COLD_MNEMONIC: &str =
    "letter advice cage absurd amount doctor acoustic avoid letter advice cage above";
const HOT_MNEMONIC: &str = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

// Passed to the contract as `param::INHERITANCE_DISTANCE`, so this is the only
// place the value is defined.
//
// The original contract uses 25920 blocks, about 18 days at Liquid's
// 1-minute block time. This demo uses 1440 (1 day) to keep the mining cheap;
// mining tens of thousands of blocks on regtest risks a `nextest` timeout.
const INHERITANCE_DISTANCE: u16 = 1440;

// Every test funds a fresh covenant instance with the same amount, and the
// fee is worked out by Simplex rather than fixed here.
const FUNDING_AMOUNT: u64 = 100_000;

/// The inheritor, the cold key holder and the hot key holder, as three
/// independent Simplex wallets, together with the covenant they are bound to.
struct Parties {
    inheritor: Signer,
    cold: Signer,
    hot: Signer,
    arguments: LastWillArguments,
    program: LastWillProgram,
}

impl Parties {
    fn new(context: &simplex::TestContext) -> Self {
        let inheritor = context.create_signer(INHERITOR_MNEMONIC);
        let cold = context.create_signer(COLD_MNEMONIC);
        let hot = context.create_signer(HOT_MNEMONIC);

        // Parameters are compiled into the program, so these four values
        // determine the covenant's script, and therefore its address.
        let arguments = LastWillArguments {
            inheritance_distance: INHERITANCE_DISTANCE,
            inheritor_pk: inheritor.get_schnorr_public_key().serialize(),
            cold_pk: cold.get_schnorr_public_key().serialize(),
            hot_pk: hot.get_schnorr_public_key().serialize(),
        };

        Self {
            inheritor,
            cold,
            hot,
            program: LastWillProgram::new(&arguments),
            arguments,
        }
    }

    /// The covenant these three wallets are bound to. `Program` caches its
    /// compilation, so holding one instance here compiles the contract once per
    /// test rather than once per spend.
    fn program(&self) -> &LastWillProgram {
        &self.program
    }

    fn script(&self, context: &simplex::TestContext) -> Script {
        self.program.get_script_pubkey(context.get_network())
    }

    /// The same contract and the same three parties, but with a doubled
    /// inheritance distance. Changing a `param::` changes the script, so this
    /// is a valid last will covenant sitting at a different address.
    fn other_covenant_script(&self, context: &simplex::TestContext) -> Script {
        let mut arguments = self.arguments.clone();
        arguments.inheritance_distance = INHERITANCE_DISTANCE * 2;

        LastWillProgram::new(&arguments).get_script_pubkey(context.get_network())
    }
}

/// Which of the contract's three branches a spend takes.
///
/// `witness::INHERIT_OR_NOT` is an `Either<Signature, Either<Signature,
/// Signature>>`, so each spending path is a position in that nested type.
/// `witness()` puts a placeholder signature at that position, and
/// `sig_path()` names the same position for the signer, which replaces it.
/// An `enum` would read better in the contract, but Simplex does not support
/// contracts that use one.
#[derive(Clone, Copy)]
enum SpendPath {
    Inherit,
    ColdSpend,
    HotSpend,
}

impl SpendPath {
    fn witness(self) -> LastWillWitness {
        self.witness_with(DUMMY_SIGNATURE)
    }

    fn witness_with(self, signature: [u8; 64]) -> LastWillWitness {
        LastWillWitness {
            inherit_or_not: match self {
                SpendPath::Inherit => Either::Left(signature),
                SpendPath::ColdSpend => Either::Right(Either::Left(signature)),
                SpendPath::HotSpend => Either::Right(Either::Right(signature)),
            },
        }
    }

    fn sig_path(self) -> &'static [&'static str] {
        match self {
            SpendPath::Inherit => &["Left"],
            SpendPath::ColdSpend => &["Right", "Left"],
            SpendPath::HotSpend => &["Right", "Right"],
        }
    }
}

/// Where a spend sends the value it takes out of the covenant.
///
/// Only `SpendPath::HotSpend` constrains this: `recursive_covenant()` requires
/// output 0 to pay back into the covenant. The other two paths don't constrain
/// outputs, so the value can go wherever the spender wants.
enum ChangeTo {
    /// The spending party's own wallet.
    SpenderWallet,
    /// Back into the covenant itself, which is what a `HotSpend` requires.
    Covenant,
    /// Some other script, which a `HotSpend` must reject.
    Other(Script),
}

/// An Elements fee output. Unlike Bitcoin, Elements/Liquid represents the
/// transaction fee as an explicit output with an empty scriptPubKey, which
/// `recursive_covenant()` checks for at output index 1 with
/// `jet::output_is_fee(1)`.
fn fee_output(amount: u64, network: &SimplicityNetwork) -> PartialOutput {
    PartialOutput::new(Script::new(), amount, network.policy_asset())
}

/// Funds a fresh covenant instance, then spends it via `path`, signed by
/// `signer`.
///
/// Returns the local-execution or broadcast error (if any) unexamined, so
/// callers can assert either success or a specific failure result.
fn spend_action(
    context: &simplex::TestContext,
    parties: &Parties,
    description: &str,
    path: SpendPath,
    sequence: Sequence,
    signer: &Signer,
    change_to: ChangeTo,
) -> anyhow::Result<Txid> {
    let prog = parties.program();
    let script = prog.get_script_pubkey(context.get_network());

    let utxo = fund_covenant(context, &script)?;

    let change_to = match change_to {
        ChangeTo::SpenderWallet => signer.get_address().script_pubkey(),
        ChangeTo::Covenant => script,
        ChangeTo::Other(other) => other,
    };
    let ft = covenant_spend(prog, utxo, path, sequence, change_to);

    println!("Submitting {description}...");

    // One call: coin selection, fee rate, the change and fee outputs, local
    // execution, signing, witness satisfaction and broadcast. A test that needs
    // to control those steps uses `spend_with_exact_outputs` instead.
    let receipt = signer.broadcast(&ft)?;
    println!("  -> accepted: {receipt}");
    Ok(receipt.txid())
}

/// Builds a spend of `utxo` through `path`, sending the value to `change_to`.
///
/// The transaction carries one input and no explicit outputs: `Signer` appends
/// the change output and then the Elements fee output, and works out both
/// amounts. For a `SpendPath::HotSpend` with `change_to` set to the covenant's
/// own script, those two appended outputs are what `recursive_covenant()`
/// requires: output 0 back into the covenant, output 1 the fee.
fn covenant_spend(
    prog: &LastWillProgram,
    utxo: UTXO,
    path: SpendPath,
    sequence: Sequence,
    change_to: Script,
) -> FinalTransaction {
    let mut ft = FinalTransaction::new();

    ft.add_program_input(
        PartialInput::new(utxo).with_sequence(sequence),
        ProgramInput::new(Box::new(prog.as_ref().clone()), Box::new(path.witness())),
        RequiredSignature::witness_with_path("INHERIT_OR_NOT", path.sig_path()),
    );
    ft.add_change(ChangeOutput::new(change_to));

    ft
}

/// Funds a fresh instance of the covenant at `script` and returns its UTXO,
/// once the funding transaction has confirmed.
///
/// Without the `TxReceipt::wait` the UTXO comes back out of the mempool, which
/// works, but leaves each test depending on relay timing rather than a block.
fn fund_covenant(context: &simplex::TestContext, script: &Script) -> anyhow::Result<UTXO> {
    let receipt = context.get_default_signer().send(script.clone(), FUNDING_AMOUNT)?;
    let txid = receipt.txid();

    receipt.wait()?;

    covenant_utxo(context, script, txid)
}

/// The UTXO that transaction `txid` created at `script`.
///
/// Selecting by txid avoids picking up whichever other UTXO the covenant's
/// script happens to hold.
fn covenant_utxo(context: &simplex::TestContext, script: &Script, txid: Txid) -> anyhow::Result<UTXO> {
    context
        .get_default_provider()
        .fetch_scripthash_utxos(script)?
        .into_iter()
        .find(|utxo| utxo.outpoint.txid == txid)
        .ok_or_else(|| anyhow::anyhow!("transaction {txid} produced no UTXO at the covenant script"))
}

/// Spends a covenant UTXO with a transaction whose outputs are given exactly
/// as `outputs`, with no change output and no automatic fee output.
///
/// Every caller is a test that expects the spend to be rejected. Spends that
/// are meant to succeed go through `spend_action`.
///
/// The output-shape tests below violate `recursive_covenant()` on purpose: one
/// output instead of two, a non-fee output at index 1, and the fee and
/// continuation outputs swapped. `spend_action` cannot express those, because
/// `Signer::broadcast` would append its own change and fee outputs on top and
/// trip the `num_outputs() == 2` check instead of the one under test.
/// `Signer::sign_tx` appends nothing, so these transactions are balanced by
/// hand.
fn spend_with_exact_outputs(
    context: &simplex::TestContext,
    parties: &Parties,
    description: &str,
    path: SpendPath,
    sequence: Sequence,
    signer: &Signer,
    outputs: Vec<PartialOutput>,
) -> anyhow::Result<Txid> {
    let prog = parties.program();
    let script = prog.get_script_pubkey(context.get_network());

    let utxo = fund_covenant(context, &script)?;

    let mut ft = FinalTransaction::new();
    ft.add_program_input(
        PartialInput::new(utxo).with_sequence(sequence),
        ProgramInput::new(Box::new(prog.as_ref().clone()), Box::new(path.witness())),
        RequiredSignature::witness_with_path("INHERIT_OR_NOT", path.sig_path()),
    );
    for output in outputs {
        ft.add_output(output);
    }

    println!("Submitting {description}...");

    // Signing and broadcast are kept apart deliberately: `sign_tx` runs the
    // Simplicity program, where a failed `assert!` surfaces, before the
    // transaction reaches the network and its consensus rules, such as relative
    // locktimes. Several tests depend on telling those two apart.
    let tx = signer.sign_tx(&ft)?;
    let receipt = context.get_default_provider().broadcast_transaction(&tx)?;
    println!("  -> accepted: {receipt}");
    Ok(receipt.txid())
}

/// Asserts that a spend helper's result failed during local Simplicity
/// execution rather than succeeding or failing only once it reached a
/// node. Every test that expects a spend to be rejected outright (wrong
/// key, or a `recursive_covenant()` violation) fails this way: the
/// contract's own logic catches it before broadcast is ever attempted.
///
/// Both spend helpers run the program through the `Signer`, so a failed
/// `assert!` always arrives as `SignerError::CovenantExecution`. Matching the
/// type rather than the message text keeps the test independent of a
/// dependency's wording.
fn expect_assert_failure(result: anyhow::Result<Txid>, what_was_wrong: &str) -> anyhow::Result<()> {
    match result {
        Ok(txid) => {
            anyhow::bail!("expected local execution to reject this ({what_was_wrong}), but it broadcast: {txid}")
        }
        Err(err) => {
            anyhow::ensure!(
                matches!(err.downcast_ref::<SignerError>(), Some(SignerError::CovenantExecution { .. })),
                "expected an assertion failure ({what_was_wrong}), got a different error: {err}"
            );
            println!("  -> rejected locally, as expected: {what_was_wrong}\n");
            Ok(())
        }
    }
}

#[simplex::test]
fn cold_spend_breaks_out_of_the_covenant(context: simplex::TestContext) -> anyhow::Result<()> {
    let parties = Parties::new(&context);

    println!(
        "\n=== ColdSpend: the owner cancels the covenant entirely ===\n\n\
         `cold_spend` is the simplest of the three paths, just a signature check\n\
         against `param::COLD_PK`. Whoever holds the cold private key\n\
         can withdraw the funds from this covenant at any time, ending it.\n"
    );

    spend_action(
        &context,
        &parties,
        "ColdSpend transaction, signed with the cold key",
        SpendPath::ColdSpend,
        Sequence::default(),
        &parties.cold,
        ChangeTo::SpenderWallet,
    )?;
    Ok(())
}

#[simplex::test]
fn hot_spend_refreshes_the_covenant(context: simplex::TestContext) -> anyhow::Result<()> {
    let parties = Parties::new(&context);

    println!(
        "\n=== HotSpend: the owner checks in, and the covenant is refreshed ===\n\n\
         `refresh_spend` checks a signature and then calls `recursive_covenant()`,\n\
         which requires the transaction to have two outputs. Output 0 must be a new\n\
         UTXO paying back to the same covenant script, verified by comparing\n\
         commitment hashes rather than by trusting the witness. Output 1 must be a\n\
         fee output. These two checks carry the covenant across transactions.\n\
         Bitcoin Script has no equivalent, because it cannot inspect or constrain\n\
         transaction outputs.\n\n\
         The test doesn't assemble those two outputs itself: it hands Simplex the\n\
         covenant's own script as the change target, and Simplex emits the change\n\
         output followed by the fee output, working out both amounts.\n"
    );

    spend_action(
        &context,
        &parties,
        "HotSpend transaction, signed with the hot key",
        SpendPath::HotSpend,
        Sequence::default(),
        &parties.hot,
        ChangeTo::Covenant,
    )?;

    println!(
        "The covenant's {INHERITANCE_DISTANCE}-block inheritance clock has effectively\n\
         been reset. The new UTXO this transaction creates is a fresh instance of the\n\
         same covenant, carrying the same rules as the original.\n"
    );
    Ok(())
}

#[simplex::test]
fn inherit_too_early_is_rejected(context: simplex::TestContext) -> anyhow::Result<()> {
    let parties = Parties::new(&context);

    println!(
        "\n=== Inherit, attempted immediately: rejected by the network ===\n\n\
         The inheritor declares a sequence number claiming the full {INHERITANCE_DISTANCE}-block\n\
         wait has passed. `enforce_relative_distance` only checks that the declared\n\
         value is large enough. It cannot see the chain state, so local execution\n\
         succeeds. No blocks are mined first, so the distance actually elapsed since\n\
         funding falls short of the declared value, and the node rejects the\n\
         transaction as too early.\n"
    );

    let result = spend_action(
        &context,
        &parties,
        &format!(
            "Inherit transaction, declaring the full {INHERITANCE_DISTANCE}-block wait, with no blocks actually mined"
        ),
        SpendPath::Inherit,
        Sequence::from_height(INHERITANCE_DISTANCE),
        &parties.inheritor,
        ChangeTo::SpenderWallet,
    );

    match result {
        Ok(txid) => {
            anyhow::bail!("expected the node to reject this broadcast, but it succeeded: {txid}")
        }
        Err(err) => {
            // `ProviderError::BroadcastRejected` carries the node's own response
            // body. Matching that variant, rather than grepping the formatted
            // error chain, keeps this test specific to a network rejection: a
            // local `assert!` failure arrives as
            // `SignerError::CovenantExecution` and must not pass here.
            let Some(SignerError::Provider(ProviderError::BroadcastRejected { message, .. })) =
                err.downcast_ref::<SignerError>()
            else {
                anyhow::bail!("expected the node to reject this broadcast, got a different error: {err}")
            };

            anyhow::ensure!(
                message.contains("non-BIP68-final"),
                "expected a `non-BIP68-final` rejection specifically, got: {message}"
            );
            println!("  -> rejected by the node: {message}\n");
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
    let parties = Parties::new(&context);

    println!(
        "\n=== Inherit successfully: fast-forwarding {INHERITANCE_DISTANCE} blocks ===\n\n\
         At Liquid's 1-minute block time, {INHERITANCE_DISTANCE} blocks is about a day,\n\
         against the 25920 blocks (~18 days) the upstream contract asks for. In a\n\
         real deployment, the inheritor has to wait the correct amount of time\n\
         before inheriting. For demonstration purposes, we can fast-forward the\n\
         network by asking the regtest to mine the required number of blocks\n\
         immediately, ignoring its default blocktime delay.\n"
    );

    let prog = parties.program();
    let script = prog.get_script_pubkey(context.get_network());

    // The relative timelock runs from the height the funding output confirmed
    // at, so the target below has to be measured from a tip that contains it.
    let utxo = fund_covenant(&context, &script)?;

    let target = context.get_default_provider().fetch_tip_height()? as u64 + u64::from(INHERITANCE_DISTANCE);
    context.get_network_utils().mine_until_height(target)?;
    println!("Mined up to height {target}.\n");

    let to = parties.inheritor.get_address().script_pubkey();
    let ft = covenant_spend(prog, utxo, SpendPath::Inherit, Sequence::from_height(INHERITANCE_DISTANCE), to);

    println!("Submitting the same Inherit transaction as before, now that the wait is real...");

    let receipt = parties.inheritor.broadcast(&ft)?;
    println!("  -> accepted: {receipt}");

    println!(
        "\nThe same spend that was rejected before now succeeds. The contract and\n\
         witness are the same, but the blockchain state has changed.\n"
    );
    Ok(())
}

// =====================================================================
// Wrong-key tests: each of the three paths checks a signature against a
// specific one of the contract's three `param::` public keys. These tests
// deliberately sign with the wrong party's wallet and so fail during local
// execution.
// =====================================================================

#[simplex::test]
fn hot_spend_with_cold_key_fails(context: simplex::TestContext) -> anyhow::Result<()> {
    let parties = Parties::new(&context);

    println!(
        "\n=== HotSpend, signed with the cold key instead of the hot key ===\n\n\
         Correctly structured HotSpend, but signed by the cold wallet instead of\n\
         the hot wallet, so the signature does not verify against `param::HOT_PK`.\n"
    );
    let result = spend_action(
        &context,
        &parties,
        "HotSpend transaction, signed with the cold key instead of the hot key",
        SpendPath::HotSpend,
        Sequence::default(),
        &parties.cold,
        ChangeTo::Covenant,
    );
    expect_assert_failure(result, "cold key signed a HotSpend")
}

#[simplex::test]
fn hot_spend_with_inheritor_key_fails(context: simplex::TestContext) -> anyhow::Result<()> {
    let parties = Parties::new(&context);

    println!(
        "\n=== HotSpend, signed with the inheritor's key instead of the hot key ===\n\n\
         Same idea as the cold-key case, but with the inheritor's wallet: the\n\
         inheritor cannot act as the owner and bypass the timelock.\n"
    );
    let result = spend_action(
        &context,
        &parties,
        "HotSpend transaction, signed with the inheritor's key instead of the hot key",
        SpendPath::HotSpend,
        Sequence::default(),
        &parties.inheritor,
        ChangeTo::Covenant,
    );
    expect_assert_failure(result, "inheritor key signed a HotSpend")
}

#[simplex::test]
fn cold_spend_with_hot_key_fails(context: simplex::TestContext) -> anyhow::Result<()> {
    let parties = Parties::new(&context);

    println!(
        "\n=== ColdSpend, signed with the hot key instead of the cold key ===\n\n\
         Even though the two owner-held keys (hot and cold) belong to the same person\n\
         in practice, they aren't interchangeable and have different abilities.\n\
         Here we demonstrate this by trying to sign a ColdSpend with the hot key.\n"
    );
    let result = spend_action(
        &context,
        &parties,
        "ColdSpend transaction, signed with the hot key instead of the cold key",
        SpendPath::ColdSpend,
        Sequence::default(),
        &parties.hot,
        ChangeTo::SpenderWallet,
    );
    expect_assert_failure(result, "hot key signed a ColdSpend")
}

#[simplex::test]
fn cold_spend_with_inheritor_key_fails(context: simplex::TestContext) -> anyhow::Result<()> {
    let parties = Parties::new(&context);

    println!(
        "\n=== ColdSpend, signed with the inheritor's key ===\n\n\
         The same principle applies to the third of the three wallets.\n"
    );
    let result = spend_action(
        &context,
        &parties,
        "ColdSpend transaction, signed with the inheritor's key",
        SpendPath::ColdSpend,
        Sequence::default(),
        &parties.inheritor,
        ChangeTo::SpenderWallet,
    );
    expect_assert_failure(result, "inheritor key signed a ColdSpend")
}

#[simplex::test]
fn inherit_with_hot_key_after_wait_fails(context: simplex::TestContext) -> anyhow::Result<()> {
    let parties = Parties::new(&context);

    println!(
        "\n=== Inherit, signed with the hot key instead of the inheritor's key ===\n\n\
         Once again, the keys are not interchangeable and the key used must match the\n\
         specific spending path it is supposed to authorize.\n"
    );
    let result = spend_action(
        &context,
        &parties,
        "Inherit transaction (declared wait satisfied), signed with the hot key",
        SpendPath::Inherit,
        Sequence::from_height(INHERITANCE_DISTANCE),
        &parties.hot,
        ChangeTo::SpenderWallet,
    );
    expect_assert_failure(result, "hot key signed an Inherit")
}

#[simplex::test]
fn inherit_with_cold_key_after_wait_fails(context: simplex::TestContext) -> anyhow::Result<()> {
    let parties = Parties::new(&context);

    println!("\n=== Inherit, signed with the cold key ===\n\nThe same applies to the other owner-held key.\n");
    let result = spend_action(
        &context,
        &parties,
        "Inherit transaction (declared wait satisfied), signed with the cold key",
        SpendPath::Inherit,
        Sequence::from_height(INHERITANCE_DISTANCE),
        &parties.cold,
        ChangeTo::SpenderWallet,
    );
    expect_assert_failure(result, "cold key signed an Inherit")
}

// =====================================================================
// `recursive_covenant()` violations. Each condition enforcing the
// structure of the covenant is broken on purpose by one of these tests.
// =====================================================================

#[simplex::test]
fn hot_spend_wrong_output_count_fails(context: simplex::TestContext) -> anyhow::Result<()> {
    let parties = Parties::new(&context);

    println!(
        "\n=== HotSpend with only one output ===\n\n\
         `recursive_covenant()`'s first check is that `num_outputs()` equals 2.\n\
         Here, the entire 100,000 sats is provided as a single fee output.\n\
         Since this contract does not constrain the value of the fee output, a\n\
         stolen hot key could still leak most of the balance to miners this way.\n\
         This test only shows that skipping the continuation output entirely\n\
         is caught.\n"
    );
    let result = spend_with_exact_outputs(
        &context,
        &parties,
        "HotSpend transaction with a single (fee-only) output",
        SpendPath::HotSpend,
        Sequence::default(),
        &parties.hot,
        vec![fee_output(FUNDING_AMOUNT, context.get_network())],
    );
    expect_assert_failure(result, "HotSpend with 1 output instead of 2")
}

#[simplex::test]
fn hot_spend_wrong_continuation_script_fails(context: simplex::TestContext) -> anyhow::Result<()> {
    let parties = Parties::new(&context);

    println!(
        "\n=== HotSpend recreating the wrong script ===\n\n\
         Output 0 here pays to a real last will covenant over the same three keys,\n\
         differing only in its inheritance distance. Parameters are compiled into\n\
         the program, so that alone changes the script. `recursive_covenant()`\n\
         compares the output's script hash against this program's own (not trusting\n\
         the witness), so a HotSpend can't move the funds into a covenant on\n\
         different terms while disguised as a routine refresh.\n"
    );

    // Check the test's premise: a different parameter means a different address.
    let other = parties.other_covenant_script(&context);
    anyhow::ensure!(
        other != parties.script(&context),
        "changing `inheritance_distance` did not change the covenant's script"
    );

    let result = spend_action(
        &context,
        &parties,
        "HotSpend transaction recreating a covenant on different terms, not this one",
        SpendPath::HotSpend,
        Sequence::default(),
        &parties.hot,
        ChangeTo::Other(other),
    );
    expect_assert_failure(result, "HotSpend output 0 recreates the wrong script")
}

#[simplex::test]
fn hot_spend_non_fee_second_output_fails(context: simplex::TestContext) -> anyhow::Result<()> {
    let parties = Parties::new(&context);

    println!(
        "\n=== HotSpend with a real script where the fee output should be ===\n\n\
         Output 0 correctly recreates the covenant. Output 1, though, has a real\n\
         destination script instead of the empty scriptPubKey that marks an actual fee\n\
         output, so `jet::output_is_fee(1)` is `false` and the assertion fails.\n"
    );
    let script = parties.script(&context);
    let not_a_fee = parties.hot.get_address().script_pubkey();
    let result = spend_with_exact_outputs(
        &context,
        &parties,
        "HotSpend transaction with a non-fee second output",
        SpendPath::HotSpend,
        Sequence::default(),
        &parties.hot,
        vec![
            PartialOutput::new(script, 99_000, context.get_network().policy_asset()),
            PartialOutput::new(not_a_fee, 1_000, context.get_network().policy_asset()),
        ],
    );
    expect_assert_failure(result, "HotSpend output 1 isn't a real fee output")
}

#[simplex::test]
fn hot_spend_swapped_outputs_fails(context: simplex::TestContext) -> anyhow::Result<()> {
    let parties = Parties::new(&context);

    println!(
        "\n=== HotSpend with the fee and continuation outputs swapped ===\n\n\
         A real continuation output and a real fee output are both present, but at the\n\
         wrong indices. Both of `recursive_covenant()`'s checks are position-specific\n\
         (`output_script_hash(0)`, `output_is_fee(1)`), not \"does some output satisfy\n\
         this somewhere,\" so swapping them fails.\n"
    );
    let script = parties.script(&context);
    let result = spend_with_exact_outputs(
        &context,
        &parties,
        "HotSpend transaction with fee/continuation outputs swapped",
        SpendPath::HotSpend,
        Sequence::default(),
        &parties.hot,
        vec![
            fee_output(1_000, context.get_network()),
            PartialOutput::new(script, 99_000, context.get_network().policy_asset()),
        ],
    );
    expect_assert_failure(result, "HotSpend outputs in the wrong order")
}

// =====================================================================
// A realistic multi-hop spending chain: the covenant is refreshed twice by
// the owner and then gets broken out of with ColdSpend.
// =====================================================================

#[simplex::test]
fn covenant_survives_two_refreshes_then_breaks_out(context: simplex::TestContext) -> anyhow::Result<()> {
    let parties = Parties::new(&context);

    println!(
        "\n=== A full lifecycle: refresh, refresh again, then break out ===\n\n\
         Three chained transactions. The owner checks in twice with the hot key,\n\
         each spend consuming the UTXO the previous transaction created, so the\n\
         covenant persists across several hops. The owner then exits via the cold\n\
         key. The covenant script is the same at every step.\n"
    );

    let prog = parties.program();
    let script = prog.get_script_pubkey(context.get_network());

    let funded = fund_covenant(&context, &script)?;

    // Each hop takes the covenant UTXO it is spending and returns the fresh one
    // it created, so the chain is threaded by outpoint rather than by refetching
    // the script. The new balance comes from `UTXO::amount` on that output, so
    // the hop never needs to know the fee.
    let hot_spend_hop = |utxo: UTXO, hop_name: &str| -> anyhow::Result<UTXO> {
        println!("Submitting {hop_name} (HotSpend, {} sats in)...", utxo.amount());

        // Paying the change back into the covenant's own script gives
        // `recursive_covenant()` the outputs it requires.
        let ft = covenant_spend(prog, utxo, SpendPath::HotSpend, Sequence::default(), script.clone());

        let receipt = parties.hot.broadcast(&ft)?;
        let txid = receipt.txid();

        // Each hop must confirm before the next one can spend its output.
        receipt.wait()?;
        println!("  -> accepted and confirmed: {receipt}\n");

        covenant_utxo(&context, &script, txid)
    };

    let after_first = hot_spend_hop(funded, "first refresh")?;
    println!("Covenant still alive at {} sats.\n", after_first.amount());
    let after_second = hot_spend_hop(after_first, "second refresh")?;
    println!("Covenant still alive at {} sats.\n", after_second.amount());

    // Final hop: break out via ColdSpend, spending the UTXO the second refresh
    // created.
    let to = parties.cold.get_address().script_pubkey();
    let ft = covenant_spend(prog, after_second, SpendPath::ColdSpend, Sequence::default(), to);

    println!("Submitting final hop (ColdSpend, breaking out of the covenant for good)...");

    let receipt = parties.cold.broadcast(&ft)?;
    println!("  -> accepted: {receipt}\n");

    println!(
        "The covenant was recreated through two refresh cycles and then ended\n\
         on purpose.\n"
    );
    Ok(())
}
