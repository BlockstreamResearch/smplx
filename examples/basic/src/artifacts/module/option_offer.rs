use simplex::simplex_macros::include_simf;
use simplex::simplex_sdk::program::{ArgumentsTrait, Program};
use simplicityhl::elements::secp256k1_zkp::XOnlyPublicKey;
pub struct OptionOfferProgram {
    program: Program,
}
impl OptionOfferProgram {
    pub const SOURCE: &'static str = derived_option_offer::OPTION_OFFER_CONTRACT_SOURCE;
    pub fn new(
        public_key: XOnlyPublicKey,
        arguments: impl ArgumentsTrait + 'static,
    ) -> Self {
        Self {
            program: Program::new(Self::SOURCE, public_key, Box::new(arguments)),
        }
    }
    pub fn get_program(&self) -> &Program {
        &self.program
    }
    pub fn get_program_mut(&mut self) -> &mut Program {
        &mut self.program
    }
}
include_simf!("simf/module/option_offer.simf");
