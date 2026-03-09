use dyn_clone::DynClone;
use simplicityhl::WitnessValues;

pub trait WitnessTrait: DynClone {
    fn build_witness(&self) -> WitnessValues;
}

dyn_clone::clone_trait_object!(WitnessTrait);
