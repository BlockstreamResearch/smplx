use std::sync::Arc;
use crate::mutantesting::FuzzContext;
use crate::mutantesting::core::{FuzzableBaseContextGen, FuzzableContextGen, FuzzableProgram};
use proptest::test_runner::TestRng;
use simplicityhl::elements::hashes::Hash;
use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::elements::{OutPoint, TxOut, Txid};
use simplicityhl::{Arguments, WitnessValues};
use smplx_sdk::provider::SimplicityNetwork;
use smplx_sdk::signer::{Signer, SignerError};
use smplx_sdk::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature, UTXO};

#[derive(Default)]
pub struct DefaultBaseContextGen {}

impl<FuzzProgram: FuzzableProgram<FuzzProgram>> FuzzableBaseContextGen<FuzzProgram> for DefaultBaseContextGen {
    // TODO: move base transaction creation into Tree strategy
    fn build_base_transaction(
        &self,
        network: &SimplicityNetwork,
        args: Arguments,
        wit: WitnessValues,
    ) -> FinalTransaction {
        const DEFAULT_FAUCET: u64 = 1 << 32;

        let mut ft = FinalTransaction::new();

        let (failure_program, failure_script) = FuzzProgram::build_program(args, network);

        let txout = {
            let mut r = TxOut::new_fee(DEFAULT_FAUCET, network.policy_asset());
            r.script_pubkey = failure_script;
            r
        };

        ft.add_program_input(
            PartialInput::new(UTXO {
                outpoint: OutPoint::new(Txid::all_zeros(), 0),
                txout,
                secrets: None,
            }),
            ProgramInput::new(Box::new(failure_program.get_program().clone()), wit),
            RequiredSignature::None,
        );

        ft
    }

    fn build_base_transaction_2(
        &self,
        context: &FuzzContext,
        args: Arguments,
        wit: WitnessValues,
        rng: &mut TestRng,
    ) -> (FinalTransaction, PartiallySignedTransaction, Arguments, WitnessValues) {
        todo!("FuzzableBaseContextGen::build_base_transaction_2")
    }
}

#[derive(Default)]
pub struct DefaultContextGen {}

impl<FuzzProgram: FuzzableProgram<FuzzProgram>> FuzzableContextGen<FuzzProgram> for DefaultContextGen {
    // todo: move into one strategy
    fn modify_transaction(
        &self,
        _signer: &Option<Arc<Signer>>,
        ft: FinalTransaction,
        _args: &Arguments,
        _wit: &WitnessValues,
    ) -> Result<PartiallySignedTransaction, SignerError> {
        Ok(ft.extract_pst().0)
        // TODO: fix, incorrect witness_utxo extraction from sign_tx method
        //  idea - to sign and retrieve a valid finalized transaction to check, but not partial one without fees
        // match signer {
        //     None => Ok(ft.extract_pst().0),
        //     Some(s) => Ok(s.finalize_offline(&ft)?.0),
        // }
    }
}
