use std::collections::HashMap;
use std::fmt::{Display, Formatter, Result as FmtResult};

use proptest::test_runner::RngSeed;

use crate::fuzz::outcome::{Outcome, Rejection};
use crate::fuzz::spec::Expect;

/// What a fuzz run exercised, printed when the run ends.
pub struct RunSummary {
    test_name: &'static str,
    seed: RngSeed,
    layout: String,
    cases: usize,
    accepted: usize,
    rejections: HashMap<Rejection, usize>,
    rejected_by: Vec<(String, usize)>,
    expectations: HashMap<Expect, usize>,
    check_auth: bool,
    auth_checks: usize,
}

impl RunSummary {
    pub fn new(test_name: &'static str, seed: RngSeed, layout: String, check_auth: bool) -> Self {
        Self {
            test_name,
            seed,
            layout,
            cases: 0,
            accepted: 0,
            rejections: HashMap::new(),
            rejected_by: Vec::new(),
            expectations: HashMap::new(),
            check_auth,
            auth_checks: 0,
        }
    }

    /// `rejected_by` names whoever rejected the case.
    pub fn record(&mut self, expect: Expect, verdict: Outcome, rejected_by: Option<String>) {
        self.cases += 1;
        *self.expectations.entry(expect).or_default() += 1;

        match verdict {
            Outcome::Accepted => self.accepted += 1,
            Outcome::Rejected(rejection) => *self.rejections.entry(rejection).or_default() += 1,
        }

        if let Some(name) = rejected_by {
            match self.rejected_by.iter_mut().find(|(rejector, _)| *rejector == name) {
                Some((_, count)) => *count += 1,
                None => self.rejected_by.push((name, 1)),
            }
        }
    }

    pub fn record_auth_check(&mut self) {
        self.auth_checks += 1;
    }

    pub fn is_run_empty(&self, allow_unspendable: bool) -> Option<String> {
        if self.expectations.get(&Expect::Any) == Some(&self.cases) {
            return Some(format!(
                "The spec made no prediction in {} case(s), it returned `Expect::Any` for every run",
                self.cases
            ));
        }

        if self.accepted == 0 && !allow_unspendable {
            return Some(format!(
                "No case was accepted in {} run(s), so either the spec is faulty or the covenant is unspendable. \
                 Consider calling `allow_unspendable()` if unspendability is intended",
                self.cases
            ));
        }

        None
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

        writeln!(f, "fuzz {}: {} case(s), seed {seed}", self.test_name, self.cases)?;
        writeln!(f, "  programs       {}", self.layout)?;
        writeln!(
            f,
            "  verdicts       accepted {}, rejected {} ({rejections})",
            self.accepted,
            self.cases - self.accepted
        )?;

        if !self.rejected_by.is_empty() {
            let rejected_by = self
                .rejected_by
                .iter()
                .map(|(name, count)| format!("{name} {count}"))
                .collect::<Vec<_>>()
                .join(", ");

            writeln!(f, "  rejected by    {rejected_by}")?;
        }

        writeln!(
            f,
            "  spec           accept {}, reject {}, any {}",
            Self::count(&self.expectations, &Expect::Accept),
            Self::count(&self.expectations, &Expect::Reject),
            Self::count(&self.expectations, &Expect::Any)
        )?;

        if self.check_auth {
            write!(
                f,
                "  authorization  {} signed cases checked with a wrong key and a replayed signature",
                self.auth_checks
            )
        } else {
            write!(f, "  authorization  off")
        }
    }
}
