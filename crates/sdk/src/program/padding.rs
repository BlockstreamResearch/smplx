//! Automatic witness padding for newly deployed programs.
//!
//! Prepare a program through [`Signer::prepare_program`](crate::signer::Signer::prepare_program) before funding its address.
//! The framework commits a fixed padding prefix and supplies its zero witness during execution.
//! Application witness types stay unchanged.
//!
//! ```
//! use smplx_sdk::program::{ArgumentsTrait, Program};
//! use smplx_sdk::signer::{Signer, SignerError};
//!
//! # fn prepare(signer: &Signer, arguments: &impl ArgumentsTrait) -> Result<Program, SignerError> {
//! let program = signer.prepare_program(Program::new("fn main() {}", arguments))?;
//! # Ok(program)
//! # }
//! ```
//!
//! Preparation changes the CMR and address.
//! It cannot retrofit an already funded program.
//! Padding uses the unpruned execution bound, including the padding prefix's own cost.
//! The signer checks the final witness budget and includes padding in transaction fees.
//! [`MAX_PADDING_BYTES`] limits the fixed padding witness.
//! The signer also rejects transactions above 400,000 weight units.
//!
//! The public entry points are [`Signer::prepare_program`](crate::signer::Signer::prepare_program) and [`Program::with_automatic_padding`](super::Program::with_automatic_padding).
//! DAG construction stays internal so callers cannot detach padding from its program commitment.

use std::sync::Arc;

use simplicityhl::ast::ElementsJetHinter;
use simplicityhl::simplicity::bitcoin::Weight;
use simplicityhl::simplicity::node::{Inner, RedeemData, SimpleFinalizer};
use simplicityhl::simplicity::{CommitNode, RedeemNode};
use simplicityhl::{Arguments, CompiledProgram, UnstableFeatures};

use super::ProgramError;

/// Maximum fixed padding witness size in bytes.
///
/// The framework chooses the size automatically up to this limit of 256 KiB.
/// This leaves room for the application, outputs and proofs under standard transaction limits.
/// Preparation returns [`ProgramError::PaddingLimit`] when its conservative bound cannot fit.
pub const MAX_PADDING_BYTES: usize = 262_144;

/// A fixed, committed prefix whose witness is supplied by the framework.
pub(super) struct Padding {
    prefix: Arc<RedeemNode>,
    pub(super) commitment: Arc<CommitNode>,
}

impl Padding {
    pub(super) fn new(program: &CommitNode) -> Result<Self, ProgramError> {
        // Finalize without pruning.
        // Bounds include the most expensive branch even when dummy zero witnesses would fail execution.
        let unpruned = program
            .finalize(&mut SimpleFinalizer::new(std::iter::empty()))
            .map_err(|err| ProgramError::Padding(err.to_string()))?;

        if Weight::from(unpruned.bounds().cost).to_wu() > MAX_PADDING_BYTES as u64 {
            return Err(ProgramError::PaddingLimit {
                max_bytes: MAX_PADDING_BYTES,
            });
        }
        for words in (0..=13).map(|power| 1usize << power) {
            let bytes = words * 32;
            debug_assert!(bytes <= MAX_PADDING_BYTES);
            let prefix = compile_prefix(words)?;
            // The budget proof depends on these bytes surviving serialization.
            if prefix.to_vec_with_witness().1.len() != bytes {
                return Err(ProgramError::Padding(
                    "compiler changed the padding witness width".into(),
                ));
            }
            let composed = compose(&prefix, &unpruned);
            let cost = composed.bounds().cost;
            // Count only the fixed padding bytes.
            // Serialized program bytes, application witnesses and stack overhead provide additional budget.
            if cost.is_consensus_valid() && Weight::from(cost).to_wu() <= bytes as u64 {
                let commitment = composed
                    .unfinalize()
                    .map_err(|err| ProgramError::Padding(err.to_string()))?;
                return Ok(Self { prefix, commitment });
            }
        }
        Err(ProgramError::PaddingLimit {
            max_bytes: MAX_PADDING_BYTES,
        })
    }

    pub(super) fn apply(&self, program: &Arc<RedeemNode>) -> Arc<RedeemNode> {
        compose(&self.prefix, program)
    }
}

fn compile_prefix(words: usize) -> Result<Arc<RedeemNode>, ProgramError> {
    let source = format!(
        "fn consume(word: u256, valid: bool) -> bool {{\n\
            assert!(jet::eq_256(word, 0)); valid\n\
        }}\n\
        fn main() {{\n\
            assert!(array_fold::<consume, {words}>(witness::PADDING, true));\n\
        }}"
    );
    CompiledProgram::new_with_unstable(
        source,
        &UnstableFeatures::all(),
        Arguments::default(),
        false,
        Box::new(ElementsJetHinter),
    )
    .map_err(ProgramError::Padding)?
    .commit()
    .finalize(&mut SimpleFinalizer::new(std::iter::empty()))
    .map_err(|err| ProgramError::Padding(err.to_string()))
}

// Both operands are compiled programs with the finalized type unit -> unit.
// Compose after HL optimization so the padding cannot disappear as unused code.
fn compose(left: &Arc<RedeemNode>, right: &Arc<RedeemNode>) -> Arc<RedeemNode> {
    assert!(left.arrow().source.is_unit() && left.arrow().target.is_unit());
    assert!(right.arrow().source.is_unit() && right.arrow().target.is_unit());
    let data = RedeemData::new(
        right.arrow().shallow_clone(),
        Inner::Comp(left.cached_data(), right.cached_data()),
    );
    Arc::new(RedeemNode::from_parts(
        Inner::Comp(Arc::clone(left), Arc::clone(right)),
        Arc::new(data),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use simplicityhl::elements::encode::serialize;

    #[test]
    fn matching_zero_application_witness_keeps_the_padding_budget() {
        use crate::program::{ArgumentsTrait, Program, ProgramTrait};
        use crate::provider::SimplicityNetwork;
        use simplicityhl::elements::pset::{Input, PartiallySignedTransaction};
        use simplicityhl::elements::{TxOut, confidential};
        use simplicityhl::num::U256;
        use simplicityhl::simplicity::bitcoin::secp256k1::{Keypair, Message, SECP256K1, SecretKey};
        use simplicityhl::str::WitnessName;
        use simplicityhl::types::TypeConstructible;
        use simplicityhl::value::{UIntValue, ValueConstructible};
        use simplicityhl::{ResolvedType, Value, WitnessValues};
        use std::collections::HashMap;

        #[derive(Clone)]
        struct Empty;
        impl ArgumentsTrait for Empty {
            fn build_arguments(&self) -> Arguments {
                Arguments::default()
            }
        }
        let key = Keypair::from_secret_key(SECP256K1, &SecretKey::from_slice(&[1; 32]).unwrap());
        let signature = SECP256K1.sign_schnorr(&Message::from_digest([0; 32]), &key);
        let check = format!("jet::bip_0340_verify((0x{}, 0), signature);", key.x_only_public_key().0);
        let mut reproduced_sharing = false;
        for words in [8, 16, 32, 64, 128] {
            let source = format!(
                "fn consume(word: u256, valid: bool) -> bool {{ assert!(jet::eq_256(word, 0)); valid }}\n\
                fn main() {{\n\
                    assert!(array_fold::<consume, {words}>(witness::APPLICATION_ZEROS, true));\n\
                    let signature: Signature = witness::SIGNATURE; {}\n\
                }}",
                check.repeat(8)
            );
            let compiled = CompiledProgram::new_with_unstable(
                source.clone(),
                &UnstableFeatures::all(),
                Arguments::default(),
                false,
                Box::new(ElementsJetHinter),
            )
            .unwrap();
            let padding = Padding::new(&compiled.commit()).unwrap();
            let padding_bytes = padding.prefix.to_vec_with_witness().1.len();
            if padding_bytes != words * 32 {
                continue;
            }

            let program = Program::new(source, &Empty)
                .with_debug_symbols(false)
                .with_automatic_padding();
            program.prepare().unwrap();
            let network = SimplicityNetwork::default_regtest();
            let mut pst = PartiallySignedTransaction::new_v2();
            pst.add_input(Input {
                witness_utxo: Some(TxOut {
                    asset: confidential::Asset::Explicit(network.policy_asset()),
                    value: confidential::Value::Explicit(100_000),
                    script_pubkey: program.get_script_pubkey(&network),
                    ..TxOut::default()
                }),
                ..Input::default()
            });
            let witness = WitnessValues::from(HashMap::from([
                (
                    WitnessName::from_str_unchecked("SIGNATURE"),
                    Value::byte_array(signature.serialize()),
                ),
                (
                    WitnessName::from_str_unchecked("APPLICATION_ZEROS"),
                    Value::array(
                        (0..words).map(|_| Value::from(UIntValue::U256(U256::from_byte_array([0; 32])))),
                        ResolvedType::u256(),
                    ),
                ),
            ]));
            let stack = program.finalize(&pst, &witness, 0, &network).unwrap();
            // The application and prefix share one zero witness rather than retaining two.
            assert_eq!(stack[0].len(), padding_bytes + 64);
            let redeem = program.execute(&pst, &witness, 0, &network).unwrap().0;
            assert!(redeem.bounds().cost.is_budget_valid(&stack));
            assert_eq!(redeem.cmr(), padding.commitment.cmr());
            reproduced_sharing = true;
        }
        assert!(reproduced_sharing, "fixture must exercise exact type-and-value sharing");
    }

    fn signature_check() -> Arc<RedeemNode> {
        CompiledProgram::new_with_unstable(
            "fn main() { jet::bip_0340_verify((0, jet::sig_all_hash()), witness::SIGNATURE); }",
            &UnstableFeatures::all(),
            Arguments::default(),
            false,
            Box::new(ElementsJetHinter),
        )
        .unwrap()
        .commit()
        .finalize(&mut SimpleFinalizer::new(std::iter::empty()))
        .unwrap()
    }

    #[test]
    fn serialized_padding_survives_sharing_and_compact_size_boundaries() {
        let mut application = signature_check();
        let mut saw_short = false;
        let mut saw_long = false;
        for _ in 0..6 {
            let padding = Padding::new(&application.unfinalize().unwrap()).unwrap();
            let (_, prefix_witness) = padding.prefix.to_vec_with_witness();
            assert!(prefix_witness.iter().all(|byte| *byte == 0));
            assert!(prefix_witness.len().is_power_of_two());
            assert!(prefix_witness.len() >= 32);
            let composed = padding.apply(&application);
            let (program, witness) = composed.to_vec_with_witness();
            // Each application invocation has a 64-byte witness.
            // Equal values may share a node in the encoding.
            // The framework prefix must never shrink.
            assert!(witness.len() >= prefix_witness.len());
            assert!(Weight::from(composed.bounds().cost).to_wu() <= prefix_witness.len() as u64);
            let stack = vec![witness, program, composed.cmr().as_ref().to_vec(), vec![0; 33]];
            assert!(composed.bounds().cost.is_budget_valid(&stack));
            let encoded = serialize(&stack[0]);
            if stack[0].len() < 253 {
                saw_short = true;
                assert_eq!(encoded.len(), stack[0].len() + 1);
            } else {
                saw_long = true;
                assert_eq!(encoded.len(), stack[0].len() + 3);
            }
            application = compose(&application, &application);
        }
        assert!(saw_short && saw_long);
    }

    #[test]
    fn rejects_bounds_above_the_fixed_padding_limit() {
        let mut application = signature_check();
        while Weight::from(application.bounds().cost).to_wu() <= MAX_PADDING_BYTES as u64 {
            application = compose(&application, &application);
        }
        let commitment = application.unfinalize().unwrap();
        assert!(matches!(
            Padding::new(&commitment),
            Err(ProgramError::PaddingLimit { .. })
        ));
    }

    #[test]
    fn prefix_commitments_are_pinned_to_the_reviewed_compiler() {
        // SimplicityHL 0.7.2 with simplicity-lang 0.8.0.
        let expected = [
            (
                "cea3ceabee56875c8ce3f473851ae03e69fcbe9df0d9a5ae5528a6ea6561bc7c",
                4_782,
            ),
            (
                "0c189e5537112c728adc9a1328051b4322643cc09ed6d960ee3dfbd6aecbb3bc",
                10_431,
            ),
            (
                "a9a8b39f0376b07f00bfa4ef32640915b898ff9ec77f02c829c5acb31b991ccc",
                22_753,
            ),
            (
                "2a3371f988faeb31f9bfe807828a2299668c40914974adbf6e961773ee19019d",
                49_445,
            ),
            (
                "516ce4727e58d3df5d878591197830b655cbc3c039a7edc44a2627662710e3a0",
                106_925,
            ),
            (
                "53948e253fe2ae91f2c03bab4f31f0230070f859969cdf3c80be1c3571ec8f83",
                230_077,
            ),
            (
                "048b62cf12321970ec3ea3e418c457325e269b1769d9ec4ff373bf86d9d21bde",
                492_765,
            ),
            (
                "05b973f5bcddf4e5ec57b7c1dc5ac866ddc1a126e8189e243008d0c992eedd51",
                1_050_909,
            ),
            (
                "17195a995675da85bba6f627c0527094e87bb2659cd4523c912ba7d302afb3d2",
                2_232_733,
            ),
            (
                "7fbe81de3de178777225e69154c47c67049213c313043ba4752655d7a13786d2",
                4_727_453,
            ),
            (
                "9f1f5e24869232f2968274a9bee51098b6632f3e975cc0623f2381fe83d9bf76",
                9_979_037,
            ),
            (
                "bedddd8e0b4e1a335ff00436b251506b359c42541ebb72893723c6eaa926a008",
                21_006_493,
            ),
            (
                "17762d909470acf3d8122a5c6a72a5d75b5fe5ae4f1ec1a6a8fb454e15ae0564",
                44_109_981,
            ),
            (
                "d96bab809bdc2fbecdd572afae5b200fc39406dc39af03383a765487cf13889b",
                92_414_109,
            ),
        ];
        for (power, (cmr, milliweight)) in expected.iter().enumerate() {
            let words = 1 << power;
            let prefix = compile_prefix(words).unwrap();
            assert_eq!(prefix.to_vec_with_witness().1.len(), words * 32);
            assert_eq!(
                prefix.cmr().to_string(),
                *cmr,
                "prefix identity changed for {words} words"
            );
            assert_eq!(
                prefix.bounds().cost,
                simplicityhl::simplicity::Cost::from_milliweight(*milliweight)
            );
        }
    }

    #[test]
    fn upper_prefix_sizes_cover_the_composed_cost_and_reject_the_effective_limit() {
        let check = signature_check();
        let mut application = Arc::clone(&check);
        for _ in 0..10 {
            application = compose(&application, &application);
        }
        for expected_bytes in [Some(131_072), Some(262_144), None] {
            let commitment = application.unfinalize().unwrap();
            match (Padding::new(&commitment), expected_bytes) {
                (Ok(padding), Some(bytes)) => {
                    assert_eq!(padding.prefix.to_vec_with_witness().1.len(), bytes);
                    assert!(
                        padding
                            .apply(&application)
                            .bounds()
                            .cost
                            .is_budget_valid(&vec![vec![0; bytes]])
                    );
                    simplicityhl::simplicity::BitMachine::for_program(&padding.prefix).unwrap();
                }
                (Err(ProgramError::PaddingLimit { .. }), None) => {
                    // The original bound fits 256 KiB, but the prefix's own cost does not.
                    assert!(Weight::from(application.bounds().cost).to_wu() <= MAX_PADDING_BYTES as u64);
                }
                _ => panic!("unexpected upper-limit padding result for {expected_bytes:?}"),
            }
            application = compose(&application, &application);
        }
    }
}
