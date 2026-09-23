use super::*;
use crate::program::{ArgumentsTrait, ProgramError, WitnessTrait};
use crate::transaction::{PartialInput, ProgramInput, UTXO};
use simplicityhl::Arguments;
use simplicityhl::elements::{OutPoint, TxOut, Txid, confidential};
use simplicityhl::simplicity::BitMachine;

#[derive(Clone)]
struct Empty;

impl ArgumentsTrait for Empty {
    fn build_arguments(&self) -> Arguments {
        Arguments::default()
    }
}
impl WitnessTrait for Empty {
    fn build_witness(&self) -> WitnessValues {
        WitnessValues::default()
    }
}

fn signer() -> Signer {
    Signer::from_mnemonic(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        SimplicityNetwork::default_regtest(),
    )
}

fn signature_program(signer: &Signer, checks: usize) -> Program {
    let public_key = hex::encode(signer.get_schnorr_public_key().serialize());
    let verify = format!("jet::bip_0340_verify((0x{public_key}, jet::sig_all_hash()), signature);");
    Program::new(
        format!(
            "fn main() {{ let signature: Signature = witness::SIGNATURE; {} }}",
            verify.repeat(checks)
        ),
        &Empty,
    )
    .with_debug_symbols(false)
}

fn transaction(signer: &Signer, program: &Program, signature: RequiredSignature) -> FinalTransaction {
    let asset = signer.network.policy_asset();
    let mut tx = FinalTransaction::new();
    tx.add_program_input(
        PartialInput::new(UTXO {
            outpoint: OutPoint::new(Txid::from_byte_array([1; 32]), 0),
            txout: TxOut {
                asset: confidential::Asset::Explicit(asset),
                value: confidential::Value::Explicit(100_000),
                script_pubkey: program.get_script_pubkey(&signer.network),
                ..TxOut::default()
            },
            secrets: None,
        }),
        ProgramInput::new(Box::new(program.clone()), Box::new(Empty)),
        signature,
    );
    tx.add_output(PartialOutput::new(signer.get_address().script_pubkey(), 50_000, asset));
    tx.add_change(ChangeOutput::new(signer.get_address().script_pubkey()));
    tx
}

fn decode(stack: &[Vec<u8>]) -> Arc<RedeemNode> {
    RedeemNode::decode::<_, _, Elements>(BitIter::from(stack[1].as_slice()), BitIter::from(stack[0].as_slice()))
        .unwrap()
}

#[test]
fn prepares_padding_without_application_witness_fields_and_signs_final_transaction() {
    let signer = signer();
    let original = signature_program(&signer, 8);
    let original_cmr = original.get_cmr();
    let program = signer.prepare_program(original.clone()).unwrap();
    assert_ne!(program.get_cmr(), original_cmr);
    assert_eq!(original.get_cmr(), original_cmr);
    assert_eq!(program.get_witness_types().unwrap().iter().count(), 1);
    assert_eq!(
        signer.prepare_program(program.clone()).unwrap().get_cmr(),
        program.get_cmr()
    );

    let tx = transaction(&signer, &program, RequiredSignature::Witness("SIGNATURE".into()));
    let (finalized, fee) = signer.finalize_strict(&tx, 1000.0).unwrap();
    let stack = &finalized.input[0].witness.script_witness;
    let redeem = decode(stack);
    assert_eq!(stack.len(), 4);
    assert!(stack[0].len() > 64);
    assert!(redeem.bounds().cost.is_budget_valid(stack));
    assert_eq!(redeem.cmr().as_ref(), program.get_cmr());
    assert!(fee >= tx.calculate_fee(finalized.discount_weight(), 1000.0));
    // Rebuild the environment from the exact returned outputs and signatures.
    let mut pst = PartiallySignedTransaction::from_tx(finalized.clone());
    pst.inputs_mut()[0].witness_utxo = Some(tx.inputs()[0].partial_input.witness_utxo.clone());
    let env = program.get_env(&pst, 0, &signer.network).unwrap();
    BitMachine::for_program(&redeem).unwrap().exec(&redeem, &env).unwrap();

    let mut changed = finalized;
    changed.output[0].value = confidential::Value::Explicit(49_999);
    let mut wrong_pst = PartiallySignedTransaction::from_tx(changed);
    wrong_pst.inputs_mut()[0].witness_utxo = Some(tx.inputs()[0].partial_input.witness_utxo.clone());
    let wrong_env = program.get_env(&wrong_pst, 0, &signer.network).unwrap();
    assert!(
        BitMachine::for_program(&redeem)
            .unwrap()
            .exec(&redeem, &wrong_env)
            .is_err()
    );
}

#[test]
fn rejects_old_underfunded_program_and_cannot_retrofit_its_address() {
    let signer = signer();
    let original = signature_program(&signer, 8);
    let tx = transaction(&signer, &original, RequiredSignature::Witness("SIGNATURE".into()));
    assert!(matches!(
        signer.sign_tx(&tx),
        Err(SignerError::CovenantExecution {
            source: ProgramError::InsufficientBudget,
            ..
        })
    ));
    let prepared = signer.prepare_program(original).unwrap();
    let (pst, _) = tx.extract_pst();
    assert!(matches!(
        prepared.get_env(&pst, 0, &signer.network),
        Err(ProgramError::ScriptPubkeyMismatch { .. })
    ));
}

#[test]
fn finalizer_rejects_corrupt_serialized_witness() {
    let signer = signer();
    let program = signer.prepare_program(signature_program(&signer, 8)).unwrap();
    let tx = transaction(&signer, &program, RequiredSignature::Witness("SIGNATURE".into()));
    let finalized = signer.sign_tx(&tx).unwrap();
    let mut stack = finalized.input[0].witness.script_witness.clone();
    stack[2][0] ^= 1;
    assert!(Signer::validate_program_witness(0, &stack).is_err());
    stack.pop();
    assert!(Signer::validate_program_witness(0, &stack).is_err());
}

#[test]
fn rejects_nonzero_padding_without_changing_the_commitment() {
    let signer = signer();
    let program = signer.prepare_program(signature_program(&signer, 8)).unwrap();
    let tx = transaction(&signer, &program, RequiredSignature::Witness("SIGNATURE".into()));
    let finalized = signer.sign_tx(&tx).unwrap();
    let mut stack = finalized.input[0].witness.script_witness.clone();
    stack[0][0] ^= 0x80;
    let redeem = decode(&stack);
    assert_eq!(redeem.cmr().as_ref(), program.get_cmr());
    let (pst, _) = tx.extract_pst();
    let env = program.get_env(&pst, 0, &signer.network).unwrap();
    assert!(BitMachine::for_program(&redeem).unwrap().exec(&redeem, &env).is_err());
}

#[derive(Clone)]
struct Branch(bool);
impl WitnessTrait for Branch {
    fn build_witness(&self) -> WitnessValues {
        WitnessValues::from(HashMap::from([(
            WitnessName::from_str_unchecked("HEAVY"),
            Value::from(self.0),
        )]))
    }
}

#[test]
fn branches_share_one_address_and_always_keep_the_full_padding_witness() {
    let signer = signer();
    let public_key = hex::encode(signer.get_schnorr_public_key().serialize());
    let check = format!("jet::bip_0340_verify((0x{public_key}, jet::sig_all_hash()), signature);");
    let source = format!(
        "fn main() {{ let signature: Signature = witness::SIGNATURE; {check}\n\
        match witness::HEAVY {{ true => {{ {} }}, false => {{ }} }} }}",
        check.repeat(8)
    );
    let program = signer
        .prepare_program(Program::new(source, &Empty).with_debug_symbols(false))
        .unwrap();
    let mut sizes = Vec::new();
    let mut costs = Vec::new();
    for heavy in [false, true] {
        let mut tx = transaction(&signer, &program, RequiredSignature::Witness("SIGNATURE".into()));
        tx.inputs_mut()[0].program_input.as_mut().unwrap().witness = Box::new(Branch(heavy));
        let finalized = signer.sign_tx(&tx).unwrap();
        let stack = &finalized.input[0].witness.script_witness;
        let redeem = decode(stack);
        assert_eq!(redeem.cmr().as_ref(), program.get_cmr());
        assert!(redeem.bounds().cost.is_budget_valid(stack));
        sizes.push(stack[0].len());
        costs.push(redeem.bounds().cost);
    }
    assert_eq!(sizes[0], sizes[1]);
    assert!(costs[0] < costs[1]);
}

#[test]
fn fee_shortfall_and_standard_transaction_weight_are_rejected() {
    let signer = signer();
    let program = signer.prepare_program(signature_program(&signer, 8)).unwrap();
    let mut tx = transaction(&signer, &program, RequiredSignature::Witness("SIGNATURE".into()));
    tx.outputs_mut()[0].amount = 99_980;
    assert!(matches!(
        signer.finalize_strict(&tx, 1000.0),
        Err(SignerError::NotEnoughFeeAmount(..))
    ));
    tx.outputs_mut()[0].script_pubkey = Script::from(vec![0; 100_001]);
    assert!(matches!(
        signer.sign_tx(&tx),
        Err(SignerError::TransactionTooLarge { .. })
    ));
}

#[test]
fn debug_mode_change_rebuilds_the_padding_commitment() {
    let signer = signer();
    let plain = signer.prepare_program(signature_program(&signer, 1)).unwrap();
    let debug = plain.clone().with_debug_symbols(true);
    debug.prepare().unwrap();
    assert_ne!(plain.get_cmr(), debug.get_cmr());
    let tx = transaction(&signer, &debug, RequiredSignature::Witness("SIGNATURE".into()));
    assert!(signer.sign_tx(&tx).is_ok());
}

#[test]
fn existing_low_cost_programs_need_no_opt_in_or_padding() {
    let signer = signer();
    let program = Program::new("fn main() { assert!(true); }", &Empty).with_debug_symbols(false);
    let cmr = program.get_cmr();
    let tx = transaction(&signer, &program, RequiredSignature::None);
    let finalized = signer.sign_tx(&tx).unwrap();
    let stack = &finalized.input[0].witness.script_witness;
    assert!(stack[0].is_empty());
    assert_eq!(stack[2], cmr);
    assert_eq!(program.get_cmr(), cmr);
}
