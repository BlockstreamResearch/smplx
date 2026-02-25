use simplex::simplex_macros::include_simf;
use simplex::simplex_sdk::arguments::ArgumentsTrait;
use simplex::simplex_sdk::program::Program;
use simplicityhl::simplicity::bitcoin::XOnlyPublicKey;

pub struct Options<'a> {
    program: Program<'a>,
}

impl<'a> Options<'a> {
    pub const SOURCE: &'static str = derived_options::OPTIONS_CONTRACT_SOURCE;

    pub fn new(public_key: &'a XOnlyPublicKey, arguments: &'a impl ArgumentsTrait) -> Self {
        Self {
            program: Program::new(Self::SOURCE, public_key, arguments),
        }
    }

    pub fn get_program(&self) -> &Program<'a> {
        &self.program
    }

    pub fn get_program_mut(&mut self) -> &mut Program<'a> {
        &mut self.program
    }
}

include_simf!("./simf/options.simf");
