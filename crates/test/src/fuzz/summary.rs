use std::collections::HashMap;
use std::fmt::{Display, Formatter, Result as FmtResult};

use proptest::test_runner::RngSeed;

use crate::fuzz::outcome::{Outcome, Rejection};
use crate::fuzz::spec::Expect;

/// What a fuzz run exercised, printed when the run ends.
pub struct RunSummary {
    test_name: &'static str,
    seed: RngSeed,
    cases: usize,
    accepted: usize,
    rejections: HashMap<Rejection, usize>,
    expectations: HashMap<Expect, usize>,
    check_authorization: bool,
    authorization_checks: usize,
}

impl RunSummary {
    pub fn new(test_name: &'static str, seed: RngSeed, check_authorization: bool) -> Self {
        Self {
            test_name,
            seed,
            cases: 0,
            accepted: 0,
            rejections: HashMap::new(),
            expectations: HashMap::new(),
            check_authorization,
            authorization_checks: 0,
        }
    }

    pub fn record(&mut self, expect: Expect, verdict: Outcome) {
        self.cases += 1;
        *self.expectations.entry(expect).or_default() += 1;

        match verdict {
            Outcome::Accepted => self.accepted += 1,
            Outcome::Rejected(rejection) => *self.rejections.entry(rejection).or_default() += 1,
        }
    }

    pub fn record_authorization(&mut self) {
        self.authorization_checks += 1;
    }

    /// Why a run without findings still tested nothing, if it did not.
    pub fn empty_run(&self, allow_no_acceptance: bool) -> Option<String> {
        if self.expectations.get(&Expect::Any) == Some(&self.cases) {
            return Some(format!(
                "The spec made no prediction in {}, it returned `Expect::Any` for every case",
                self.cases()
            ));
        }

        if self.accepted == 0 && !allow_no_acceptance {
            return Some(format!(
                "No case was accepted in {}, so acceptance was never tested and every rejection may come \
                 from the setup. Call `allow_no_acceptance()` if no spend should ever be accepted",
                self.cases()
            ));
        }

        None
    }

    fn cases(&self) -> String {
        match self.cases {
            1 => "1 case".to_string(),
            cases => format!("{cases} cases"),
        }
    }

    fn count<K: Eq + std::hash::Hash>(counts: &HashMap<K, usize>, key: &K) -> usize {
        counts.get(key).copied().unwrap_or_default()
    }
}

impl Display for RunSummary {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let seed = match self.seed {
            RngSeed::Fixed(seed) => format!("{seed:#018x}"),
            RngSeed::Random => "random".to_string(),
        };

        let rejections = Rejection::ALL
            .iter()
            .map(|rejection| format!("{rejection} {}", Self::count(&self.rejections, rejection)))
            .collect::<Vec<_>>()
            .join(", ");

        writeln!(f, "fuzz {}: {}, seed {seed}", self.test_name, self.cases())?;
        writeln!(
            f,
            "  verdicts  accepted {}, rejected {} ({rejections})",
            self.accepted,
            self.cases - self.accepted
        )?;
        writeln!(
            f,
            "  spec      accept {}, reject {}, any {}",
            Self::count(&self.expectations, &Expect::Accept),
            Self::count(&self.expectations, &Expect::Reject),
            Self::count(&self.expectations, &Expect::Any)
        )?;

        if self.check_authorization {
            write!(
                f,
                "  authorization  {} signed accepted cases checked with a wrong key and a replayed signature",
                self.authorization_checks
            )
        } else {
            write!(f, "  authorization  off")
        }
    }
}
