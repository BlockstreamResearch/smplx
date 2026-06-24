use std::sync::Arc;

use simplicityhl::elements::Script;
use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::simplicity::{RedeemNode, Value};
use simplicityhl::{Arguments, WitnessValues};

use smplx_sdk::program::{Program, ProgramError, ProgramFactory};
use smplx_sdk::provider::{ProviderTrait, SimplicityNetwork};
use smplx_sdk::signer::{Signer, SignerError};
use smplx_sdk::transaction::FinalTransaction;

use crate::context::TestContext;

#[derive(Clone, Debug)]
pub(crate) enum SignerOption {
    DefaultTestConfigSigner,
    CustomSigner,
    NoSigning,
}

#[derive(Clone)]
pub struct FuzzContext {
    pub(crate) signer: Arc<Option<Signer>>,
    pub mock_provider: Arc<dyn ProviderTrait>,
    pub(crate) test_context: Arc<Option<TestContext>>,
    pub(crate) signer_option: SignerOption,
    pub network: SimplicityNetwork,
}

pub type ProgramExecResult = Result<(Arc<RedeemNode>, Value), ProgramError>;

pub trait FuzzableProgram<P: AsRef<Program>>: AsRef<Program> {
    fn build_program(args: impl Into<Arguments>, network: &SimplicityNetwork) -> (Box<P>, Script);
}

impl<P: AsRef<Program> + ProgramFactory<P>> FuzzableProgram<P> for P {
    fn build_program(args: impl Into<Arguments>, network: &SimplicityNetwork) -> (Box<P>, Script) {
        let prog = P::instantiate_program(args);
        let script = prog.as_ref().as_ref().get_script_pubkey(network);
        (prog, script)
    }
}

impl FuzzContext {
    pub fn get_signer(&self) -> Option<&Signer> {
        match self.signer_option {
            SignerOption::DefaultTestConfigSigner => Some(
                self.test_context
                    .as_ref()
                    .as_ref()
                    .expect("TestContext has to be unempty in order to get a default signer")
                    .get_default_signer(),
            ),
            SignerOption::CustomSigner => self.signer.as_ref().as_ref(),
            SignerOption::NoSigning => None,
        }
    }

    #[inline]
    pub fn sign_or_extract(&self, ft: &FinalTransaction) -> Result<PartiallySignedTransaction, SignerError> {
        match self.signer_option {
            SignerOption::DefaultTestConfigSigner | SignerOption::CustomSigner => {
                let signer = self.get_signer();
                Ok(signer.unwrap().sign_tx_raw(ft)?)
            }
            SignerOption::NoSigning => Ok(ft.extract_pst().0),
        }
    }
}

pub trait ContractFuzzStrategyBlueprint<Program, Args, Wit> {
    type AdditionalInput: std::fmt::Debug + 'static;

    fn get_final_tx(
        &self,
        context: &FuzzContext,
        args: &Arguments,
        wit: &WitnessValues,
        additional: &Self::AdditionalInput,
    ) -> FinalTransaction;
}

pub trait ContractFuzzStrategy<Program, Args, Wit>: std::fmt::Debug {
    type AdditionalInput: std::fmt::Debug + 'static;

    fn gen_final_transaction(
        &self,
        test_context: FuzzContext,
        arguments: Arguments,
        witness: WitnessValues,
        additional: Self::AdditionalInput,
    ) -> (Arguments, WitnessValues, FinalTransaction);
}

pub trait ProgramCheck {
    fn call(
        &self,
        ctx: &FuzzContext,
        tx: &PartiallySignedTransaction,
        arguments: &Arguments,
        witness: &WitnessValues,
        input_index: usize,
        program_exec_result: ProgramExecResult,
    ) -> Result<(), String>;
}
