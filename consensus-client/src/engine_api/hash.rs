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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keccak256_basic() {
        let data = b"Hello, World!";
        let hash = keccak256(data);
        
        assert_eq!(hash.len(), 32);
        
        // Test that different inputs produce different hashes
        let data2 = b"Hello, World!!";
        let hash2 = keccak256(data2);
        assert_ne!(hash, hash2);
    }

    #[test]
    fn test_keccak256_empty() {
        let hash = keccak256(b"");
        assert_eq!(hash.len(), 32);
        
        // Empty input should produce a specific hash
        let expected_empty_hash = [
            0xc5, 0xd2, 0x46, 0x01, 0x86, 0xf7, 0x23, 0x3c,
            0x92, 0x7e, 0x7d, 0xb2, 0xdc, 0xc7, 0x03, 0xc0,
            0xe5, 0x00, 0xb6, 0x53, 0xca, 0x82, 0x27, 0x3b,
            0x7b, 0xfa, 0xd8, 0x04, 0x5d, 0x85, 0xa4, 0x70
        ];
        assert_eq!(hash, expected_empty_hash);
    }

    #[test]
    fn test_keccak256_single_byte() {
        let data = b"A";
        let hash = keccak256(data);
        assert_eq!(hash.len(), 32);
        
        // Single byte should produce different hash than empty
        let empty_hash = keccak256(b"");
        assert_ne!(hash, empty_hash);
    }

    #[test]
    fn test_keccak256_large_input() {
        let data = vec![0u8; 10000];
        let hash = keccak256(&data);
        assert_eq!(hash.len(), 32);
        
        // Large input should produce different hash than small input
        let small_hash = keccak256(b"small");
        assert_ne!(hash, small_hash);
    }

    #[test]
    fn test_keccak256_deterministic() {
        let data = b"Test data for deterministic hashing";
        let hash1 = keccak256(data);
        let hash2 = keccak256(data);
        
        // Same input should produce same output
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_keccak256_different_inputs() {
        let data1 = b"Input 1";
        let data2 = b"Input 2";
        let data3 = b"Input 1"; // Same as data1
        
        let hash1 = keccak256(data1);
        let hash2 = keccak256(data2);
        let hash3 = keccak256(data3);
        
        // Different inputs should produce different hashes
        assert_ne!(hash1, hash2);
        
        // Same inputs should produce same hashes
        assert_eq!(hash1, hash3);
    }

    #[test]
    fn test_keccak256_unicode() {
        let data = "Hello, 世界! 🌍".as_bytes();
        let hash = keccak256(data);
        assert_eq!(hash.len(), 32);
        
        // Unicode should produce different hash than ASCII
        let ascii_hash = keccak256(b"Hello, World!");
        assert_ne!(hash, ascii_hash);
    }

    #[test]
    fn test_merkle_root_keccak_empty() {
        let empty_txs: Vec<Bytes> = vec![];
        let root = merkle_root_keccak(&empty_txs);
        
        // Should return keccak of empty bytes
        assert_eq!(root.len(), 32);
        
        // Should be same as keccak256 of empty bytes
        let expected = keccak256(&[]);
        assert_eq!(root, expected);
    }

    #[test]
    fn test_merkle_root_keccak_single_transaction() {
        let txs = vec![Bytes::from(vec![1, 2, 3, 4, 5])];
        let root = merkle_root_keccak(&txs);
        
        assert_eq!(root.len(), 32);
        
        // Should be different from empty root
        let empty_root = merkle_root_keccak(&vec![]);
        assert_ne!(root, empty_root);
        
        // Should be keccak of the single transaction
        let expected = keccak256(&txs[0]);
        assert_eq!(root, expected);
    }

    #[test]
    fn test_merkle_root_keccak_two_transactions() {
        let txs = vec![
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 6]),
        ];
        let root = merkle_root_keccak(&txs);
        
        assert_eq!(root.len(), 32);
        
        // Should be different from single transaction root
        let single_root = merkle_root_keccak(&vec![Bytes::from(vec![1, 2, 3])]);
        assert_ne!(root, single_root);
    }

    #[test]
    fn test_merkle_root_keccak_three_transactions() {
        let txs = vec![
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 6]),
            Bytes::from(vec![7, 8, 9]),
        ];
        let root = merkle_root_keccak(&txs);
        
        assert_eq!(root.len(), 32);
        
        // With odd number, last transaction should be duplicated
        // This creates a 4-leaf tree: [1,2,3], [4,5,6], [7,8,9], [7,8,9]
    }

    #[test]
    fn test_merkle_root_keccak_four_transactions() {
        let txs = vec![
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 6]),
            Bytes::from(vec![7, 8, 9]),
            Bytes::from(vec![10, 11, 12]),
        ];
        let root = merkle_root_keccak(&txs);
        
        assert_eq!(root.len(), 32);
        
        // With even number, should create perfect binary tree
        // This creates a 4-leaf tree: [1,2,3], [4,5,6], [7,8,9], [10,11,12]
    }

    #[test]
    fn test_merkle_root_keccak_odd_number_transactions() {
        let txs = vec![
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 6]),
            Bytes::from(vec![7, 8, 9]),
        ];
        let root = merkle_root_keccak(&txs);
        
        // Should handle odd number of transactions by duplicating the last one
        assert_eq!(root.len(), 32);
        
        // The algorithm should:
        // 1. Hash each transaction: [h1, h2, h3]
        // 2. Duplicate last: [h1, h2, h3, h3]
        // 3. Pair and hash: [hash(h1||h2), hash(h3||h3)]
        // 4. Final hash: hash(hash(h1||h2) || hash(h3||h3))
    }

    #[test]
    fn test_merkle_root_keccak_large_transactions() {
        let txs = vec![
            Bytes::from(vec![0u8; 1000]),
            Bytes::from(vec![1u8; 1000]),
            Bytes::from(vec![2u8; 1000]),
            Bytes::from(vec![3u8; 1000]),
        ];
        let root = merkle_root_keccak(&txs);
        
        assert_eq!(root.len(), 32);
        
        // Large transactions should still produce valid 32-byte hash
        assert_ne!(root, [0u8; 32]); // Should not be all zeros
    }

    #[test]
    fn test_merkle_root_keccak_deterministic() {
        let txs = vec![
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 6]),
            Bytes::from(vec![7, 8, 9]),
        ];
        
        let root1 = merkle_root_keccak(&txs);
        let root2 = merkle_root_keccak(&txs);
        
        // Same input should produce same output
        assert_eq!(root1, root2);
    }

    #[test]
    fn test_merkle_root_keccak_different_inputs() {
        let txs1 = vec![
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 6]),
        ];
        
        let txs2 = vec![
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 7]), // Different last byte
        ];
        
        let root1 = merkle_root_keccak(&txs1);
        let root2 = merkle_root_keccak(&txs2);
        
        // Different inputs should produce different outputs
        assert_ne!(root1, root2);
    }

    #[test]
    fn test_merkle_root_keccak_transaction_order_matters() {
        let txs1 = vec![
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 6]),
        ];
        
        let txs2 = vec![
            Bytes::from(vec![4, 5, 6]),
            Bytes::from(vec![1, 2, 3]), // Different order
        ];
        
        let root1 = merkle_root_keccak(&txs1);
        let root2 = merkle_root_keccak(&txs2);
        
        // Different order should produce different outputs
        assert_ne!(root1, root2);
    }

    #[test]
    fn test_merkle_root_keccak_edge_cases() {
        // Test with single byte transactions
        let txs = vec![
            Bytes::from(vec![0]),
            Bytes::from(vec![1]),
            Bytes::from(vec![255]),
        ];
        let root = merkle_root_keccak(&txs);
        assert_eq!(root.len(), 32);
        
        // Test with very long transactions
        let long_txs = vec![
            Bytes::from(vec![0u8; 10000]),
            Bytes::from(vec![1u8; 10000]),
        ];
        let long_root = merkle_root_keccak(&long_txs);
        assert_eq!(long_root.len(), 32);
    }

    #[test]
    fn test_merkle_root_keccak_consistency() {
        // Test that the merkle tree construction is consistent
        let txs = vec![
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 6]),
            Bytes::from(vec![7, 8, 9]),
        ];
        
        let root = merkle_root_keccak(&txs);
        
        // Verify the construction manually:
        // 1. Hash each transaction
        let h1 = keccak256(&txs[0]);
        let h2 = keccak256(&txs[1]);
        let h3 = keccak256(&txs[2]);
        
        // 2. Duplicate last (odd number)
        let h4 = h3; // Duplicate
        
        // 3. First level: hash pairs
        let mut combined = Vec::with_capacity(64);
        combined.extend_from_slice(&h1);
        combined.extend_from_slice(&h2);
        let level1_1 = keccak256(&combined);
        
        combined.clear();
        combined.extend_from_slice(&h3);
        combined.extend_from_slice(&h4);
        let level1_2 = keccak256(&combined);
        
        // 4. Final level: hash the two remaining hashes
        combined.clear();
        combined.extend_from_slice(&level1_1);
        combined.extend_from_slice(&level1_2);
        let expected_root = keccak256(&combined);
        
        assert_eq!(root, expected_root);
    }

    #[test]
    fn test_merkle_root_keccak_performance() {
        // Test with many transactions to ensure performance is reasonable
        let mut txs = Vec::new();
        for i in 0..100 {
            txs.push(Bytes::from(vec![i as u8; 100]));
        }
        
        let start = std::time::Instant::now();
        let root = merkle_root_keccak(&txs);
        let duration = start.elapsed();
        
        assert_eq!(root.len(), 32);
        
        // Should complete in reasonable time (less than 1 second)
        assert!(duration.as_millis() < 1000);
    }

    #[test]
    fn test_merkle_root_keccak_zero_transactions() {
        // Test edge case with zero transactions
        let txs: Vec<Bytes> = vec![];
        let root = merkle_root_keccak(&txs);
        
        assert_eq!(root.len(), 32);
        assert_eq!(root, keccak256(&[]));
    }

    #[test]
    fn test_merkle_root_keccak_one_transaction() {
        // Test with exactly one transaction
        let txs = vec![Bytes::from(vec![42])];
        let root = merkle_root_keccak(&txs);
        
        assert_eq!(root.len(), 32);
        assert_eq!(root, keccak256(&txs[0]));
    }
}
