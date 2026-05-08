use dyn_clone::DynClone;
use simplicityhl::Arguments;

/// An interface for structs capable of generating static argument mapping for Simplicity programs.
/// See the `include_simc!()` macro, which generates automatic `ArgumentsTrait` implementation.
pub trait ArgumentsTrait: DynClone {
    /// Compiles and returns the bound `Arguments` dict required to instantiate a program.
    fn build_arguments(&self) -> Arguments;
}

dyn_clone::clone_trait_object!(ArgumentsTrait);
