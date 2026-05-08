use dyn_clone::DynClone;
use simplicityhl::WitnessValues;

/// An interface for structs capable of generating Simplicity program witness mappings.
/// See the ` include_simc!()` macro, which generates an automatic `WitnessTrait` implementation.
pub trait WitnessTrait: DynClone {
    /// Compiles and generates the fully populated `WitnessValues` map for execution.
    fn build_witness(&self) -> WitnessValues;
}

dyn_clone::clone_trait_object!(WitnessTrait);
