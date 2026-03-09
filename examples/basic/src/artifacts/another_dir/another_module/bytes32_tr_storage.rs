use simplex::include_simf;
use simplex::simplex_sdk::program::{ArgumentsTrait, Program};
use simplicityhl::elements::secp256k1_zkp::XOnlyPublicKey;
pub struct Bytes32TrStorageProgram {
    program: Program,
}
impl Bytes32TrStorageProgram {
    pub const SOURCE: &'static str = derived_bytes32_tr_storage::BYTES32_TR_STORAGE_CONTRACT_SOURCE;
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
include_simf!("simf/another_dir/another_module/bytes32_tr_storage.simf");
