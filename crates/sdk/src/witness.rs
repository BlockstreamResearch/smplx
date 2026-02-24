pub trait WitnessTrait {
    fn build_witness(&self) -> simplicityhl::WitnessValues;
}
