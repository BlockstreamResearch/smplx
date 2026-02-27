use simplicityhl::Arguments;
use dyn_clone::DynClone;

pub trait ArgumentsTrait: DynClone {
    fn build_arguments(&self) -> Arguments;
}

dyn_clone::clone_trait_object!(ArgumentsTrait);
