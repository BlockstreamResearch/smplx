use simplex::simplex_macros::include_simf;
use simplex::simplex_sdk::program::{ArgumentsTrait, Program};
use simplicityhl::elements::secp256k1_zkp::XOnlyPublicKey;
pub struct P2pkProgram {
    program: Program,
}
impl P2pkProgram {
    pub const SOURCE: &'static str = derived_p2pk::P2PK_CONTRACT_SOURCE;
    pub fn new(public_key: XOnlyPublicKey, arguments: impl ArgumentsTrait + 'static) -> Self {
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
include_simf!("simf/p2pk.simf");
