mod beacon_block;
mod beacon_state;

pub use beacon_state::BeaconState;
pub use beacon_block::*;

pub const GENESIS_TIME: u64 = 1755000000;