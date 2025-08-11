use alloy_primitives::Bytes;
use tiny_keccak::{Hasher, Keccak};

/// helper: keccak256
fn keccak256(b: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(b);
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    out
}

/// Build a pairwise keccak merkle root over the list of tx bytes.
/// Leaves = keccak(tx_bytes). If odd, duplicate last leaf.
pub(crate) fn merkle_root_keccak(tx_bytes_list: &[Bytes]) -> [u8; 32] {
    if tx_bytes_list.is_empty() {
        // placeholder: keccak(empty)
        return keccak256(&[]);
    }

    // build leaves
    let mut layer: Vec<[u8; 32]> = tx_bytes_list.iter().map(|b| keccak256(b)).collect();

    while layer.len() > 1 {
        let mut next = Vec::with_capacity((layer.len() + 1) / 2);
        let mut i = 0usize;
        while i < layer.len() {
            if i + 1 == layer.len() {
                // duplicate last leaf
                let mut combined = Vec::with_capacity(64);
                combined.extend_from_slice(&layer[i]);
                combined.extend_from_slice(&layer[i]);
                next.push(keccak256(&combined));
            } else {
                let mut combined = Vec::with_capacity(64);
                combined.extend_from_slice(&layer[i]);
                combined.extend_from_slice(&layer[i + 1]);
                next.push(keccak256(&combined));
            }
            i += 2;
        }
        layer = next;
    }

    layer[0]
}
