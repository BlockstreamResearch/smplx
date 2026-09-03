use dyn_clone::DynClone;
use simplicityhl::WitnessValues;

/// An interface for structs capable of generating Simplicity program witness mappings.
/// See the ` include_simf!()` macro, which generates an automatic `WitnessTrait` implementation.
pub trait WitnessTrait: DynClone {
    /// Compiles and generates the fully populated `WitnessValues` map for execution.
    fn build_witness(&self) -> WitnessValues;
}

dyn_clone::clone_trait_object!(WitnessTrait);

/// An interface for the struct capable of generating proper `WitnessValues` mappings using the provided RNG.
/// See the ` include_simf!()` macro, which generates an automatic `RandomWitness` implementation.
pub trait RandomWitness: WitnessTrait {
    /// Generates a random `WitnessValues` instance using the provided RNG.
    fn generate_witness(rng: &mut dyn rand_core::RngCore) -> WitnessValues;
}
