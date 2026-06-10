use simplex::include_simf;
use simplex::mutantesting::{generate_value_by_ty, generate_value_by_ty_iterative};
use simplex::program::Program;
use simplex::program::{ArgumentsTrait, RandomArguments, RandomWitness, WitnessTrait};
use simplex::provider::SimplicityNetwork;
use simplex::simplicityhl::elements::secp256k1_zkp::XOnlyPublicKey;
use simplex::simplicityhl::elements::Script;
use simplex::simplicityhl::{Arguments, WitnessValues};
use simplex::rand::RngCore;

#[derive(Clone)]
pub struct DualCurrencyDepositProgram {
    program: Program,
}
impl DualCurrencyDepositProgram {
    pub const SOURCE: &'static str = derived_dual_currency_deposit::DUAL_CURRENCY_DEPOSIT_CONTRACT_SOURCE;
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
impl AsRef<Program> for DualCurrencyDepositProgram {
    fn as_ref(&self) -> &Program {
        &self.program
    }
}
impl AsMut<Program> for DualCurrencyDepositProgram {
    fn as_mut(&mut self) -> &mut Program {
        &mut self.program
    }
}

include_simf!("../../../../crates/simplex/tests/ui_simfs/dual_currency_deposit.simf");

fn main() -> Result<(), String> {
    let _ = test_e2e_behaviour()?;
    let _ = test_default()?;
    let _ = test_e2e_random_behaviour()?;
    let _ = test_e2e_random_value_generation_behaviour()?;

    Ok(())
}

fn test_e2e_behaviour() -> Result<(), String> {
    let original_witness = derived_dual_currency_deposit::DualCurrencyDepositWitness::default();

    let witness_values = original_witness.build_witness();
    let recovered_witness = derived_dual_currency_deposit::DualCurrencyDepositWitness::from_witness(&witness_values)?;
    assert_eq!(original_witness, recovered_witness);

    let original_arguments = derived_dual_currency_deposit::DualCurrencyDepositArguments::default();

    let arguments_values = original_arguments.build_arguments();
    let recovered_arguments =
        derived_dual_currency_deposit::DualCurrencyDepositArguments::from_arguments(&arguments_values)?;
    assert_eq!(original_arguments, recovered_arguments);

    Ok(())
}

fn test_default() -> Result<(), String> {
    assert_eq!(
        derived_dual_currency_deposit::DualCurrencyDepositWitness::default(),
        derived_dual_currency_deposit::DualCurrencyDepositWitness::default()
    );
    assert_eq!(
        derived_dual_currency_deposit::DualCurrencyDepositArguments::default(),
        derived_dual_currency_deposit::DualCurrencyDepositArguments::default()
    );
    Ok(())
}

fn test_e2e_random_behaviour() -> Result<(), String> {
    for seed in 0..32 {
        use simplex::rand::{rngs::StdRng, SeedableRng};

        let mut rng = StdRng::seed_from_u64(seed);

        let original_witness =
            derived_dual_currency_deposit::DualCurrencyDepositWitness::generate_witness_raw(&mut rng);

        let witness_values = original_witness.build_witness();
        let recovered_witness =
            derived_dual_currency_deposit::DualCurrencyDepositWitness::from_witness(&witness_values)?;
        assert_eq!(original_witness, recovered_witness);

        rng = StdRng::seed_from_u64(seed);
        let rand_raw_witness_values =
            derived_dual_currency_deposit::DualCurrencyDepositWitness::generate_witness(&mut rng);
        assert_eq!(witness_values, rand_raw_witness_values);

        rng = StdRng::seed_from_u64(seed);
        let original_arguments =
            derived_dual_currency_deposit::DualCurrencyDepositArguments::generate_arguments_raw(&mut rng);

        let arguments_values = original_arguments.build_arguments();
        let recovered_arguments =
            derived_dual_currency_deposit::DualCurrencyDepositArguments::from_arguments(&arguments_values)?;
        assert_eq!(original_arguments, recovered_arguments);

        rng = StdRng::seed_from_u64(seed);
        let rand_raw_witness_values =
            derived_dual_currency_deposit::DualCurrencyDepositArguments::generate_arguments(&mut rng);
        assert_eq!(arguments_values, rand_raw_witness_values);
    }
    Ok(())
}

fn test_e2e_random_value_generation_behaviour() -> Result<(), String> {
    for seed in 0..32 {
        use simplex::rand::{rngs::StdRng, SeedableRng};

        let mut rng = StdRng::seed_from_u64(seed);

        {
            let witness_values = derived_dual_currency_deposit::DualCurrencyDepositWitness::generate_witness(&mut rng);

            let witness_values = regenerate_witness_values(&witness_values, &mut rng);
            let _ = derived_dual_currency_deposit::DualCurrencyDepositWitness::from_witness(&witness_values)?;

            let witness_values = regenerate_witness_values_iterative(&witness_values, &mut rng);
            let _ = derived_dual_currency_deposit::DualCurrencyDepositWitness::from_witness(&witness_values)?;
        }

        {
            let arguments_values =
                derived_dual_currency_deposit::DualCurrencyDepositArguments::generate_arguments(&mut rng);

            let arguments_values = regenerate_arguments_values(&arguments_values, &mut rng);
            let _ = derived_dual_currency_deposit::DualCurrencyDepositArguments::from_arguments(&arguments_values)?;

            let arguments_values = regenerate_arguments_values_iterative(&arguments_values, &mut rng);
            let _ = derived_dual_currency_deposit::DualCurrencyDepositArguments::from_arguments(&arguments_values)?;
        }
    }
    Ok(())
}

fn regenerate_witness_values(wit: &WitnessValues, rng: &mut dyn RngCore) -> WitnessValues {
    use std::collections::HashMap;
    let mut map: HashMap<_,_> = Default::default();
    for (name, val) in wit.iter() {
        map.insert(name.clone(), generate_value_by_ty(val.ty(), rng));
    }
    WitnessValues::from(map)
}

fn regenerate_arguments_values(args: &Arguments, rng: &mut dyn RngCore) -> Arguments {
    use std::collections::HashMap;

    let mut map: HashMap<_,_> = Default::default();
    for (name, val) in args.iter() {
        map.insert(name.clone(), generate_value_by_ty(val.ty(), rng));
    }
    Arguments::from(map)
}

fn regenerate_witness_values_iterative(wit: &WitnessValues, rng: &mut dyn RngCore) -> WitnessValues {
    use std::collections::HashMap;

    let mut map: HashMap<_,_> = Default::default();
    for (name, val) in wit.iter() {
        map.insert(name.clone(), generate_value_by_ty_iterative(val.ty(), rng));
    }
    WitnessValues::from(map)
}

fn regenerate_arguments_values_iterative(args: &Arguments, rng: &mut dyn RngCore) -> Arguments {
    use std::collections::HashMap;

    let mut map: HashMap<_,_> = Default::default();
    for (name, val) in args.iter() {
        map.insert(name.clone(), generate_value_by_ty_iterative(val.ty(), rng));
    }
    Arguments::from(map)
}
