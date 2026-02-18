use simplicityhl::WitnessValues;

pub trait WitnessTrait {
    fn build_witness(&self) -> WitnessValues;
}
