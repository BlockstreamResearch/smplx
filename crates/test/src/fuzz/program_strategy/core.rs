use simplicityhl::{Arguments, WitnessValues};

use proptest::prelude::BoxedStrategy;

use crate::fuzz::program::FuzzProgramSchema;

#[derive(Clone, Debug)]
pub(crate) enum FuzzProgramStrategy {
    /// Generated together with the other guided programs of the case.
    Guided(FuzzProgramSchema),
    Random(FuzzProgramSchema),
    Other(BoxedStrategy<FuzzProgramValues>),
}

/// The values a program strategy generates for one program of a case.
#[derive(Clone, Debug, PartialEq)]
pub struct FuzzProgramValues {
    pub arguments: Arguments,
    pub witness: WitnessValues,
    /// One 32-byte value per storage slot.
    pub storage: Vec<[u8; 32]>,
}

impl From<(Arguments, WitnessValues)> for FuzzProgramValues {
    /// Values of a program without storage.
    fn from((arguments, witness): (Arguments, WitnessValues)) -> Self {
        Self {
            arguments,
            witness,
            storage: Vec::new(),
        }
    }
}
