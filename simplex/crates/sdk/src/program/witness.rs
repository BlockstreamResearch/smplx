use simplicityhl::WitnessValues;
use dyn_clone::DynClone;

pub trait WitnessTrait: DynClone {
    fn build_witness(&self) -> WitnessValues;
}

dyn_clone::clone_trait_object!(WitnessTrait);
