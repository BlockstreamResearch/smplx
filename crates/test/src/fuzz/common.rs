use std::fmt::Debug;

use proptest::strategy::ValueTree;

/// A generated case that is never shrunk.
pub struct FixedValueTree<T>(pub T);

impl<T: Clone + Debug> ValueTree for FixedValueTree<T> {
    type Value = T;

    fn current(&self) -> T {
        self.0.clone()
    }

    fn simplify(&mut self) -> bool {
        false
    }

    fn complicate(&mut self) -> bool {
        false
    }
}
