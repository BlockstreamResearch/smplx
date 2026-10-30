use simplicityhl::num::U256;
use simplicityhl::{ResolvedType, TemplateProgramWitness};

use smplx_sdk::program::ProgramSchema;

use crate::fuzz::witness_path::WitnessPath;

/// What a strategy needs to know about one program.
#[derive(Clone, Debug)]
pub(crate) struct FuzzProgramSchema {
    pub(crate) argument_types: Vec<(TemplateProgramWitness, ResolvedType)>,
    pub(crate) witness_types: Vec<(TemplateProgramWitness, ResolvedType)>,
    pub(crate) literals: Vec<U256>,
    pub(crate) storage_capacity: usize,
    pub(crate) spending_paths: Vec<(TemplateProgramWitness, WitnessPath)>,
}

impl FuzzProgramSchema {
    pub(crate) fn of<P: ProgramSchema>() -> Self {
        Self {
            argument_types: P::argument_types(),
            witness_types: P::witness_types(),
            literals: P::literals(),
            storage_capacity: 0,
            spending_paths: Vec::new(),
        }
    }

    #[must_use]
    pub(crate) fn with_storage_capacity(mut self, storage_capacity: usize) -> Self {
        self.storage_capacity = storage_capacity;

        self
    }

    #[must_use]
    pub(crate) fn with_spending_paths(mut self, spending_paths: Vec<(TemplateProgramWitness, WitnessPath)>) -> Self {
        self.spending_paths = spending_paths;

        self
    }

    /// The spending path of `field`, empty when there is none.
    pub(crate) fn spending_path(&self, field: &TemplateProgramWitness) -> &WitnessPath {
        self.spending_paths
            .iter()
            .find(|(pinned, _)| pinned == field)
            .map_or(WitnessPath::empty(), |(_, path)| path)
    }
}
