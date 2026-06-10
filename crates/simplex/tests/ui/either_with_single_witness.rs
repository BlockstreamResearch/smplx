use simplex::include_simf;
use simplex::program::{ArgumentsTrait, RandomArguments, RandomWitness, WitnessTrait};

use simplex::program::Program;
use simplex::provider::SimplicityNetwork;
use simplex::simplicityhl::elements::Script;
use simplex::simplicityhl::elements::secp256k1_zkp::XOnlyPublicKey;
#[derive(Clone)]
pub struct EitherWithSingleWitnessProgram {
    program: Program,
}
impl EitherWithSingleWitnessProgram {
    pub const SOURCE: &'static str = derived_either_with_single_witness::EITHER_WITH_SINGLE_WITNESS_CONTRACT_SOURCE;
    #[must_use]
    pub fn new(arguments: impl Into<simplex::simplicityhl::Arguments>) -> Self {
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
    #[must_use]
    pub fn set_storage_at(&mut self, index: usize, new_value: [u8; 32]) {
        self.program.set_storage_at(index, new_value);
    }
    #[must_use]
    pub fn get_storage_len(&self) -> usize {
        self.program.get_storage_len()
    }
    #[must_use]
    pub fn get_storage(&self) -> &[[u8; 32]] {
        self.program.get_storage()
    }
    #[must_use]
    pub fn get_storage_at(&self, index: usize) -> [u8; 32] {
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
impl AsRef<Program> for EitherWithSingleWitnessProgram {
    fn as_ref(&self) -> &Program {
        &self.program
    }
}
impl AsMut<Program> for EitherWithSingleWitnessProgram {
    fn as_mut(&mut self) -> &mut Program {
        &mut self.program
    }
}

include_simf!("../../../../crates/simplex/tests/ui_simfs/either_with_single_witness.simf");

fn main() -> Result<(), String> {
    let _ = test_e2e_behaviour()?;
    let _ = test_default();
    let _ = test_e2e_random_behaviour();

    Ok(())
}

fn test_e2e_behaviour() -> Result<(), String> {
    let original_witness =
        derived_either_with_single_witness::EitherWithSingleWitnessWitness::default();

    let witness_values = original_witness.build_witness();
    let recovered_witness =
        derived_either_with_single_witness::EitherWithSingleWitnessWitness::from_witness(
            &witness_values,
        )?;
    assert_eq!(original_witness, recovered_witness);

    let original_arguments =
        derived_either_with_single_witness::EitherWithSingleWitnessArguments::default();

    let arguments_values = original_arguments.build_arguments();
    let recovered_arguments =
        derived_either_with_single_witness::EitherWithSingleWitnessArguments::from_arguments(
            &arguments_values,
        )?;
    assert_eq!(original_arguments, recovered_arguments);

    Ok(())
}

fn test_default() -> Result<(), String> {
    assert_eq!(
        derived_either_with_single_witness::EitherWithSingleWitnessWitness::default(),
        derived_either_with_single_witness::EitherWithSingleWitnessWitness::default()
    );
    assert_eq!(
        derived_either_with_single_witness::EitherWithSingleWitnessArguments::default(),
        derived_either_with_single_witness::EitherWithSingleWitnessArguments::default()
    );
    Ok(())
}

fn test_e2e_random_behaviour() -> Result<(), String> {
    for seed in 0..32 {
        use simplex::rand::{rngs::StdRng, SeedableRng};

        let mut rng = StdRng::seed_from_u64(seed);

        let original_witness =
            derived_either_with_single_witness::EitherWithSingleWitnessWitness::generate_witness_raw(
                &mut rng,
            );

        let witness_values = original_witness.build_witness();
        let recovered_witness =
            derived_either_with_single_witness::EitherWithSingleWitnessWitness::from_witness(
                &witness_values,
            )?;
        assert_eq!(original_witness, recovered_witness);

        rng = StdRng::seed_from_u64(seed);
        let rand_raw_witness_values =
            derived_either_with_single_witness::EitherWithSingleWitnessWitness::generate_witness(
                &mut rng,
            );
        assert_eq!(witness_values, rand_raw_witness_values);

        rng = StdRng::seed_from_u64(seed);
        let original_arguments =
            derived_either_with_single_witness::EitherWithSingleWitnessArguments::generate_arguments_raw(
                &mut rng,
            );

        let arguments_values = original_arguments.build_arguments();
        let recovered_arguments =
            derived_either_with_single_witness::EitherWithSingleWitnessArguments::from_arguments(
                &arguments_values,
            )?;
        assert_eq!(original_arguments, recovered_arguments);

        rng = StdRng::seed_from_u64(seed);
        let rand_raw_witness_values =
            derived_either_with_single_witness::EitherWithSingleWitnessArguments::generate_arguments(
                &mut rng,
            );
        assert_eq!(arguments_values, rand_raw_witness_values);
    }
    Ok(())
}
