use std::collections::HashMap;
use std::fmt::{Display, Formatter, Result as FmtResult};

use proptest::test_runner::RngSeed;

use crate::fuzz::case::Expect;
use crate::fuzz::mutations::Mutation;
use crate::fuzz::outcome::{Outcome, Rejection};

/// A fuzz run summary, printed when the run ends.
pub(crate) struct RunSummary {
    test_name: &'static str,
    seed: RngSeed,
    layout: String,
    cases: usize,
    accepted: usize,
    rejections: HashMap<Rejection, usize>,
    rejected_by: Vec<(String, usize)>,
    expectations: HashMap<Expect, usize>,
    violations: usize,
    check_lock: bool,
    lock_checks: usize,
    check_auth: bool,
    auth_checks: usize,
    mutate: bool,
    mutated: usize,
    mutation_kinds: Vec<(&'static str, usize)>,
}

impl RunSummary {
    pub(crate) fn new(
        test_name: &'static str,
        seed: RngSeed,
        layout: String,
        check_lock: bool,
        check_auth: bool,
        mutate: bool,
    ) -> Self {
        Self {
            test_name,
            seed,
            layout,
            cases: 0,
            accepted: 0,
            rejections: HashMap::new(),
            rejected_by: Vec::new(),
            expectations: HashMap::new(),
            violations: 0,
            check_lock,
            lock_checks: 0,
            check_auth,
            auth_checks: 0,
            mutate,
            mutated: 0,
            mutation_kinds: Vec::new(),
        }
    }

    /// `expect` is `None` for a case that violates a requirement.
    pub(crate) fn record(&mut self, expect: Option<Expect>, verdict: Outcome, rejected_by: Option<String>) {
        self.cases += 1;

        match expect {
            Some(expect) => *self.expectations.entry(expect).or_default() += 1,
            None if !verdict.is_accepted() => self.violations += 1,
            None => {}
        }

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

    pub(crate) fn record_mutations(&mut self, mutations: &[Mutation]) {
        if !mutations.is_empty() {
            self.mutated += 1;
        }

        for mutation in mutations {
            match self
                .mutation_kinds
                .iter_mut()
                .find(|(kind, _)| *kind == mutation.kind())
            {
                Some((_, count)) => *count += 1,
                None => self.mutation_kinds.push((mutation.kind(), 1)),
            }
        }
    }

    pub(crate) fn record_lock_check(&mut self) {
        self.lock_checks += 1;
    }

    pub(crate) fn record_auth_check(&mut self) {
        self.auth_checks += 1;
    }

    pub(crate) fn is_run_empty(&self, allow_unspendable: bool) -> Option<String> {
        if self.expectations.get(&Expect::Any) == Some(&(self.cases - self.violations)) {
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

        if self.violations > 0 {
            writeln!(
                f,
                "  requirements   {} case(s) rejected with a required value violated",
                self.violations
            )?;
        }

        if self.mutate {
            let kinds = self
                .mutation_kinds
                .iter()
                .map(|(kind, count)| format!("{kind} {count}"))
                .collect::<Vec<_>>()
                .join(", ");

            writeln!(f, "  mutations      {} case(s) mutated ({kinds})", self.mutated)?;
        } else {
            writeln!(f, "  mutations      off")?;
        }

        if self.check_lock {
            writeln!(
                f,
                "  lock           {} accepted case(s) checked with their locked outputs redirected",
                self.lock_checks
            )?;
        } else {
            writeln!(f, "  lock           off")?;
        }

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
