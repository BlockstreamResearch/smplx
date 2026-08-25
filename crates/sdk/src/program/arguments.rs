use dyn_clone::DynClone;
use simplicityhl::Arguments;

/// An interface for structs capable of generating static argument mapping for Simplicity programs.
/// See the `include_simf!()` macro, which generates automatic `ArgumentsTrait` implementation.
pub trait ArgumentsTrait: DynClone {
    /// Compiles and returns the bound `Arguments` dict required to instantiate a program.
    fn build_arguments(&self) -> Arguments;
}

dyn_clone::clone_trait_object!(ArgumentsTrait);

/// An interface for the struct capable of generating proper `Arguments` mappings using the provided RNG.
/// See the ` include_simf!()` macro, which generates an automatic `ArgumentsTrait` implementation.
pub trait RandomArguments: ArgumentsTrait {
    /// Generates a random `Arguments` instance using the provided RNG.
    fn generate_arguments(rng: &mut dyn rand_core::RngCore) -> Arguments;
}
