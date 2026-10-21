use crate::fuzz::mutations::Mutation;
use crate::fuzz::program_strategy::FuzzProgramValues;

/// The values of one case.
#[derive(Clone, Debug)]
pub(crate) struct CaseValues {
    pub(crate) programs: Vec<FuzzProgramValues>,
    pub(crate) change: CaseChange,
}

/// What a case changes besides program values.
#[derive(Clone, Debug)]
pub(crate) enum CaseChange {
    None,
    Violation(u64),
    Mutations(Vec<Mutation>),
}

impl CaseChange {
    pub(crate) fn violation(&self) -> Option<u64> {
        match self {
            Self::Violation(pick) => Some(*pick),
            Self::None | Self::Mutations(_) => None,
        }
    }

    pub(crate) fn mutations(&self) -> &[Mutation] {
        match self {
            Self::Mutations(mutations) => mutations,
            Self::None | Self::Violation(_) => &[],
        }
    }
}
