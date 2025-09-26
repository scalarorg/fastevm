pub mod beacon_block;
mod beacon_state;
mod fixed_bytes;
mod safe_arith;
mod safe_arith_iter;

pub use beacon_block::BeaconBlock;
pub use beacon_state::BeaconState;
pub use safe_arith::{ArithError, SafeArith};

pub const GENESIS_TIME: u64 = 1755000000;