pub use simplex;

mod p2pk_program {
    include!(concat!("../out_dir", "/p2pk.rs"));
}
pub use p2pk_program::*;
