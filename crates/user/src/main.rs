use simplex_build::asset_auth::AssetAuth;
use simplex_build::asset_auth::asset_auth_build::{AssetAuthArguments, AssetAuthWitness};

use simplex_sdk::utils::tr_unspendable_key;

fn main() {
    let witness = AssetAuthWitness { path: (false, 1, 1) };

    let arguments = AssetAuthArguments {
        first: 1,
        second: false,
    };

    let asset_auth = AssetAuth::new(&tr_unspendable_key(), &arguments);
}
