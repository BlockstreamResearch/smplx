use simplex::include_simf;
use simplex::program::Program;
use simplex::provider::SimplicityNetwork;
use simplex::simplicityhl::elements::secp256k1_zkp::XOnlyPublicKey;
use simplex::simplicityhl::elements::Script;
use simplex::simplicityhl::Arguments;

#[derive(Clone)]
pub struct SingleBitProgram {
    program: Program,
}
impl SingleBitProgram {
    pub const SOURCE: &'static str = derived_single_bit::SINGLE_BIT_CONTRACT_SOURCE;
    #[must_use]
    pub fn new(arguments: impl Into<Arguments>) -> Self {
        Self {
            program: Program::new(Self::SOURCE, arguments.into()),
        }
    }
    #[must_use]
    pub fn with_taproot_pubkey(mut self, pub_key: XOnlyPublicKey) -> Self {
        self.program = self.program.with_taproot_pubkey(pub_key);
        self
    }
    #[must_use]
    pub fn with_storage_capacity(mut self, capacity: usize) -> Self {
        self.program = self.program.with_storage_capacity(capacity);
        self
    }
    pub fn set_storage_at(&mut self, index: usize, new_value: impl Into<Vec<u8>>) {
        self.program.set_storage_at(index, new_value);
    }
    #[must_use]
    pub fn get_storage_len(&self) -> usize {
        self.program.get_storage_len()
    }
    #[must_use]
    pub fn get_storage(&self) -> &[Vec<u8>] {
        self.program.get_storage()
    }
    #[must_use]
    pub fn get_storage_at(&self, index: usize) -> Vec<u8> {
        self.program.get_storage_at(index)
    }
    #[must_use]
    pub fn get_script_pubkey(&self, network: &SimplicityNetwork) -> Script {
        self.program.get_script_pubkey(network)
    }
    #[must_use]
    pub fn get_script_hash(&self, network: &SimplicityNetwork) -> [u8; 32] {
        self.program.get_script_hash(network)
    }
}
impl AsRef<Program> for SingleBitProgram {
    fn as_ref(&self) -> &Program {
        &self.program
    }
}
impl AsMut<Program> for SingleBitProgram {
    fn as_mut(&mut self) -> &mut Program {
        &mut self.program
    }
}

include_simf!("../../../../crates/simplex/tests/ui_simfs/single_bit.simf");

fn main() -> Result<(), String> {
    let _ = test_e2e_behaviour()?;
    let _ = test_default()?;

    Ok(())
}

fn test_e2e_behaviour() -> Result<(), String> {
    for (bit, flag) in [(1, 1), (1, 0), (0, 1), (0, 0)] {
        let original_witness = derived_single_bit::SingleBitWitness { bit };

        let witness_values = (&original_witness).into();
        let recovered_witness = derived_single_bit::SingleBitWitness::from_witness(&witness_values)?;
        assert_eq!(original_witness, recovered_witness);

        let original_arguments = derived_single_bit::SingleBitArguments { flag };

        let arguments_values = (&original_arguments).into();
        let recovered_arguments = derived_single_bit::SingleBitArguments::from_arguments(&arguments_values)?;
        assert_eq!(original_arguments, recovered_arguments);
    }

    Ok(())
}

fn test_default() -> Result<(), String> {
    assert_eq!(
        derived_single_bit::SingleBitWitness::default(),
        derived_single_bit::SingleBitWitness::default()
    );
    assert_eq!(
        derived_single_bit::SingleBitArguments::default(),
        derived_single_bit::SingleBitArguments::default()
    );
    Ok(())
}
