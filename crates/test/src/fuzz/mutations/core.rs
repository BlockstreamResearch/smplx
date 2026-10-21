/// Which mutations a run draws.
#[derive(Clone, Debug)]
pub struct Mutations {
    pub(crate) amounts: bool,
    pub(crate) assets: bool,
    pub(crate) timelocks: bool,
    pub(crate) layout: bool,
    pub(crate) everywhere: bool,
}

impl Default for Mutations {
    fn default() -> Self {
        Self {
            amounts: true,
            assets: true,
            timelocks: true,
            layout: false,
            everywhere: false,
        }
    }
}

impl Mutations {
    /// No kind, to enable them one by one.
    pub fn none() -> Self {
        Self {
            amounts: false,
            assets: false,
            timelocks: false,
            layout: false,
            everywhere: false,
        }
    }

    /// Transfers value between outputs of the same asset.
    pub fn amounts(mut self) -> Self {
        self.amounts = true;

        self
    }

    /// Pays outputs in other assets.
    pub fn assets(mut self) -> Self {
        self.assets = true;

        self
    }

    /// Sets the locktime and input sequences.
    pub fn timelocks(mut self) -> Self {
        self.timelocks = true;

        self
    }

    /// Adds attacker UTXOs and reorders existing UTXOs.
    pub fn layout(mut self) -> Self {
        self.layout = true;

        self
    }

    /// Mutates every UTXO, not only programs that "spend" or "lock".
    pub fn everywhere(mut self) -> Self {
        self.everywhere = true;

        self
    }
}
