/// Which mutations a run draws.
#[derive(Clone, Debug, Default)]
pub struct Mutations {
    pub(crate) everywhere: bool,
}

impl Mutations {
    /// Mutates every UTXO, not only programs that "spend" or "lock".
    pub fn everywhere(mut self) -> Self {
        self.everywhere = true;

        self
    }
}
