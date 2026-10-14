use simplicityhl::{Arguments, WitnessValues};

use proptest::prelude::BoxedStrategy;

use crate::fuzz::program_strategy::GuidedSchema;

#[derive(Clone, Debug)]
pub(crate) enum ProgramStrategy {
    /// Generated together with the other guided programs of the case.
    Guided(GuidedSchema),
    Random(BoxedStrategy<(Arguments, WitnessValues)>),
    Other(BoxedStrategy<(Arguments, WitnessValues)>),
}
