# Proof Kinds Explained: Per Transaction Proof Calculation

## Overview

For each transaction in a block, the system calculates **Merkle Patricia Trie (MPT) proofs** to prove the state changes. There are **3 kinds of proof calculations** that occur, with different triggering mechanisms and purposes.

## Proof Kinds Summary

| Proof Kind | Purpose | When Calculated | Frequency per TX |
|------------|---------|-----------------|------------------|
| **StorageProof** | Complete storage trie proof for account's storage slots | Upfront, batched | 1 per account (deduplicated) |
| **BlindedAccountNode** | Individual account trie node | Lazy, on-demand during proof calculation | Variable (typically 0-10 nodes) |
| **BlindedStorageNode** | Individual storage trie node | Lazy, on-demand during proof calculation | Variable (typically 0-20 nodes) |

**Key Insight**: Not all proofs are calculated per transaction. Proofs are **deduplicated** across all transactions in a block, so if 100 transactions touch the same account, only **1 StorageProof** is calculated for that account.

---

## 1. StorageProof (Primary Proof)

### Purpose
Calculates a **complete multiproof** for all storage slots of a specific account. This is the main proof calculation that proves the state of an account's storage trie.

### What It Contains
- **Storage trie nodes**: All nodes needed to prove the storage slots
- **Storage leaf nodes**: The actual storage slot values
- **Branch nodes**: Nodes in the storage trie path
- **Root hash**: The storage root hash for the account

### When It's Calculated
- **Trigger**: After transaction execution, when state changes are known
- **Timing**: Upfront, before account trie walk
- **Deduplication**: One proof per unique account touched by transactions

### Flow for StorageProof

```
┌─────────────────────────────────────────────────────────────────┐
│ Transaction Execution                                           │
│ - TX1 touches Account A (storage slots: 0x1, 0x2, 0x3)         │
│ - TX2 touches Account B (storage slots: 0x5)                   │
│ - TX3 touches Account A again (storage slots: 0x4)              │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ Proof Target Deduplication                                      │
│ - Account A: [0x1, 0x2, 0x3, 0x4] (merged from TX1 + TX3)     │
│ - Account B: [0x5] (from TX2)                                   │
│ Result: 2 StorageProof requests (not 3!)                       │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ ParallelProof::decoded_multiproof()                             │
│ - Spawns StorageProof for Account A                            │
│ - Spawns StorageProof for Account B                             │
│ - All spawned immediately (non-blocking)                        │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ ProofTaskManager::queue_proof_task()                            │
│ - Queues StorageProof(Account A, slots=[0x1,0x2,0x3,0x4])     │
│ - Queues StorageProof(Account B, slots=[0x5])                   │
│ - Both queued to ProofTaskManager                               │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ ProofTaskTx::storage_proof() [Executed in parallel]              │
│                                                                   │
│ For Account A:                                                    │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ 1. Create cursor factories (DB transaction + cache)         │ │
│ │ 2. StorageProof::new_hashed(hashed_address=Account A)      │ │
│ │ 3. Walk storage trie for Account A                          │ │
│ │ 4. Collect nodes for paths:                                 │ │
│ │    - 0x1 → nodes: [branch@0x0, branch@0x1, leaf@0x1]       │ │
│ │    - 0x2 → nodes: [branch@0x0, branch@0x2, leaf@0x2]       │ │
│ │    - 0x3 → nodes: [branch@0x0, branch@0x3, leaf@0x3]       │ │
│ │    - 0x4 → nodes: [branch@0x0, branch@0x4, leaf@0x4]       │ │
│ │ 5. Deduplicate nodes (shared branches)                       │ │
│ │ 6. Encode to RLP format                                      │ │
│ │ 7. Decode to DecodedStorageMultiProof                        │ │
│ │ 8. Send result via channel                                   │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                   │
│ Similar process for Account B (parallel execution)               │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ Result: DecodedStorageMultiProof                                 │
│ - Contains all storage trie nodes                               │
│ - Contains storage root hash                                    │
│ - Ready for account trie integration                            │
└─────────────────────────────────────────────────────────────────┘
```

### Example: Transaction Touching 3 Accounts

**Transaction**: Contract call that:
- Reads from Account A (slots: 0x1, 0x2)
- Writes to Account B (slot: 0x5)
- Reads from Account C (slot: 0x10)

**StorageProofs Generated**:
- 1 StorageProof for Account A (slots: 0x1, 0x2)
- 1 StorageProof for Account B (slot: 0x5)
- 1 StorageProof for Account C (slot: 0x10)

**Total**: 3 StorageProof calculations (not per transaction, but per unique account)

---

## 2. BlindedAccountNode (Lazy Account Trie Node)

### Purpose
Retrieves **individual account trie nodes** on-demand during proof calculation. Used when walking the account trie and encountering nodes that need to be revealed.

### What It Contains
- **Single trie node**: One node from the account trie
- **Node type**: Can be branch, extension, or leaf node
- **Path**: The nibbles path to this node in the trie

### When It's Calculated
- **Trigger**: During account trie walk, when a node is not in cache
- **Timing**: Lazy, on-demand during `ParallelProof::decoded_multiproof()`
- **Frequency**: Typically 0-10 nodes per account proof, depending on trie depth

### Flow for BlindedAccountNode

```
┌─────────────────────────────────────────────────────────────────┐
│ ParallelProof::decoded_multiproof() - Account Trie Walk         │
│ - Walking account trie to build account subtree proof           │
│ - Encountering nodes that need to be revealed                  │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ HashBuilder::add_branch() or add_leaf()                        │
│ - Needs to reveal a node at path: 0x1234...                    │
│ - Node not in cache, not in in-memory updates                   │
│ - Must fetch from database                                      │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ ProofTaskTrieNodeProvider::trie_node()                          │
│ - Called by ProofTrieNodeProviderFactory                        │
│ - Creates channel for result                                    │
│ - Queues BlindedAccountNode(path=0x1234...)                     │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ ProofTaskManager::queue_proof_task()                            │
│ - Queues BlindedAccountNode(path=0x1234...)                     │
│ - Returns receiver channel                                      │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ ProofTaskTx::blinded_account_node() [Executed in parallel]      │
│                                                                   │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ 1. Create cursor factories                                  │ │
│ │ 2. Create ProofTrieNodeProviderFactory                     │ │
│ │ 3. Get account node provider                                │ │
│ │ 4. Call trie_node(path=0x1234...)                           │ │
│ │ 5. Walk trie from root to path:                            │ │
│ │    - Start at root node                                     │ │
│ │    - Follow path 0x1234... one nibble at a time             │ │
│ │    - Read intermediate nodes from DB                        │ │
│ │    - Return node at path (branch/extension/leaf)            │ │
│ │ 6. Check if node is "blinded" (in prefix set)               │ │
│ │ 7. If blinded, reveal it (decode from DB)                   │ │
│ │ 8. Return RevealedNode                                      │ │
│ │ 9. Send result via channel                                  │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                   │
│ Note: This blocks the account trie walk until node is ready     │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ Result: RevealedNode                                             │
│ - Contains the trie node data                                   │
│ - Used to continue account trie walk                            │
│ - Added to HashBuilder                                          │
└─────────────────────────────────────────────────────────────────┘
```

### Example: Account Trie Walk

**Scenario**: Building proof for 5 accounts in a block

**Account Trie Structure**:
```
Root (0x)
  └─ Branch (0x1)
      ├─ Branch (0x12)
      │   ├─ Leaf (0x123) → Account A
      │   └─ Leaf (0x124) → Account B
      └─ Branch (0x13)
          ├─ Leaf (0x135) → Account C
          └─ Leaf (0x136) → Account D
```

**BlindedAccountNode Requests**:
- Node at path `0x1` (root's child)
- Node at path `0x12` (branch node)
- Node at path `0x13` (branch node)
- **Total**: ~3-5 node requests (shared nodes are cached)

---

## 3. BlindedStorageNode (Lazy Storage Trie Node)

### Purpose
Retrieves **individual storage trie nodes** on-demand during storage proof calculation. Used when walking a specific account's storage trie.

### What It Contains
- **Single trie node**: One node from a storage trie (specific to an account)
- **Node type**: Branch, extension, or leaf node
- **Path**: The nibbles path in the storage trie
- **Account**: The hashed account address this storage trie belongs to

### When It's Calculated
- **Trigger**: During storage proof calculation, when a node is not in cache
- **Timing**: Lazy, on-demand during `StorageProof::storage_multiproof()`
- **Frequency**: Typically 0-20 nodes per storage proof, depending on storage slot distribution

### Flow for BlindedStorageNode

```
┌─────────────────────────────────────────────────────────────────┐
│ ProofTaskTx::storage_proof() - Storage Trie Walk                │
│ - Calculating storage proof for Account A                       │
│ - Walking storage trie for slots: [0x1, 0x2, 0x3]             │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ StorageProof::storage_multiproof()                              │
│ - Walking storage trie from root                                │
│ - Encountering nodes that need to be revealed                  │
│ - Node not in cache, not in in-memory updates                   │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ ProofTaskTrieNodeProvider::trie_node()                          │
│ - Called by ProofTrieNodeProviderFactory                       │
│ - Creates channel for result                                    │
│ - Queues BlindedStorageNode(account=Account A, path=0x5...)     │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ ProofTaskManager::queue_proof_task()                            │
│ - Queues BlindedStorageNode(account=Account A, path=0x5...)     │
│ - Returns receiver channel                                      │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ ProofTaskTx::blinded_storage_node() [Executed in parallel]      │
│                                                                   │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ 1. Create cursor factories                                  │ │
│ │ 2. Create ProofTrieNodeProviderFactory                     │ │
│ │ 3. Get storage node provider for Account A                 │ │
│ │ 4. Call trie_node(path=0x5...)                             │ │
│ │ 5. Walk storage trie from root to path:                    │ │
│ │    - Start at storage root (from account)                  │ │
│ │    - Follow path 0x5... one nibble at a time                │ │
│ │    - Read intermediate nodes from DB                        │ │
│ │    - Return node at path (branch/extension/leaf)            │ │
│ │ 6. Check if node is "blinded" (in prefix set)               │ │
│ │ 7. If blinded, reveal it (decode from DB)                  │ │
│ │ 8. Return RevealedNode                                      │ │
│ │ 9. Send result via channel                                  │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                   │
│ Note: This blocks the storage proof calculation until ready      │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ Result: RevealedNode                                             │
│ - Contains the storage trie node data                            │
│ - Used to continue storage trie walk                             │
│ - Added to storage proof nodes                                   │
└─────────────────────────────────────────────────────────────────┘
```

### Example: Storage Proof for Account with 3 Slots

**Storage Trie Structure** (for Account A):
```
Storage Root (0x)
  └─ Branch (0x0)
      ├─ Leaf (0x01) → Slot 0x1 value
      ├─ Leaf (0x02) → Slot 0x2 value
      └─ Branch (0x03)
          └─ Leaf (0x035) → Slot 0x3 value
```

**BlindedStorageNode Requests**:
- Node at path `0x0` (root's child)
- Node at path `0x03` (branch node)
- **Total**: ~2-4 node requests (shared nodes are cached)

---

## Complete Flow: Transaction → Proof Calculation

### Example: Single Transaction

**Transaction**: ERC20 transfer
- **From**: Account A (0xAAA...)
- **To**: Account B (0xBBB...)
- **Token Contract**: Account C (0xCCC...)
- **State Changes**:
  - Account A: Decrease balance (storage slot 0x0)
  - Account C: Update balance mapping (slots: 0xAAA... and 0xBBB...)

### Proof Calculation Flow

```
┌─────────────────────────────────────────────────────────────────┐
│ Step 1: Transaction Execution                                   │
│ - Execute TX, track state changes                              │
│ - Account A: slot 0x0 modified                                 │
│ - Account C: slots 0xAAA... and 0xBBB... modified              │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ Step 2: Proof Target Collection                                 │
│ - Collect proof targets:                                        │
│   Account A: [slot 0x0]                                         │
│   Account C: [slot 0xAAA..., slot 0xBBB...]                    │
│ - Deduplicate with other transactions in block                  │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ Step 3: Spawn StorageProofs (Upfront)                          │
│ - Queue StorageProof(Account A, slots=[0x0])                   │
│ - Queue StorageProof(Account C, slots=[0xAAA..., 0xBBB...])   │
│ - Both queued immediately, executed in parallel                │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ Step 4: StorageProof Calculation (Parallel)                    │
│                                                                   │
│ StorageProof for Account A:                                      │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ 1. Walk storage trie from root                              │ │
│ │ 2. Path to slot 0x0: 0x0 → branch node                      │ │
│ │ 3. If node not cached: BlindedStorageNode request           │ │
│ │ 4. Collect nodes: [branch@0x0, leaf@0x0]                   │ │
│ │ 5. Return DecodedStorageMultiProof                           │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                   │
│ StorageProof for Account C:                                      │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ 1. Walk storage trie from root                              │ │
│ │ 2. Path to slot 0xAAA...: 0xAAA... → multiple nodes        │ │
│ │ 3. Path to slot 0xBBB...: 0xBBB... → multiple nodes        │ │
│ │ 4. If nodes not cached: BlindedStorageNode requests         │ │
│ │ 5. Collect and deduplicate nodes                            │ │
│ │ 6. Return DecodedStorageMultiProof                           │ │
│ └─────────────────────────────────────────────────────────────┘ │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ Step 5: Account Trie Walk (Parallel)                            │
│ - Walk account trie to build account subtree proof              │
│ - For Account A:                                                │
│   * Get StorageProof result (already calculated)                │
│   * Build account RLP with storage root                         │
│   * Add to HashBuilder                                          │
│ - For Account C:                                                │
│   * Get StorageProof result (already calculated)                │
│   * Build account RLP with storage root                         │
│   * Add to HashBuilder                                          │
│ - If account trie nodes not cached: BlindedAccountNode requests│
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ Step 6: Complete Multiproof                                     │
│ - Account subtree: All account nodes in path                    │
│ - Storage proofs: For Account A and Account C                   │
│ - Root hash: Calculated from HashBuilder                        │
└─────────────────────────────────────────────────────────────────┘
```

### Proof Count Summary

For this single transaction:
- **StorageProof**: 2 (one per account)
- **BlindedStorageNode**: ~2-6 (depends on storage trie structure)
- **BlindedAccountNode**: ~2-5 (depends on account trie structure)

**Total proof operations**: ~6-13 per transaction (but deduplicated across block)

---

## Deduplication Across Block

### Key Optimization

Proofs are **deduplicated** across all transactions in a block:

**Example**: Block with 1000 transactions
- 500 transactions touch Account A
- 300 transactions touch Account B
- 200 transactions touch Account C

**Proofs Calculated**:
- **StorageProof**: 3 (one per account, not 1000!)
- **BlindedStorageNode**: ~6-20 (shared across proofs)
- **BlindedAccountNode**: ~3-10 (shared across proofs)

**Deduplication saves**: ~997 StorageProof calculations!

---

## Performance Characteristics

### StorageProof
- **Frequency**: 1 per unique account (deduplicated)
- **Time**: 10-50ms per proof (depends on slot count)
- **Parallelism**: Up to 512 concurrent
- **Cache Impact**: High (500k node cache)

### BlindedAccountNode
- **Frequency**: 0-10 per account proof
- **Time**: 1-5ms per node
- **Parallelism**: Up to 512 concurrent
- **Cache Impact**: Medium (node-level caching)

### BlindedStorageNode
- **Frequency**: 0-20 per storage proof
- **Time**: 1-5ms per node
- **Parallelism**: Up to 512 concurrent
- **Cache Impact**: Medium (node-level caching)

---

## Code Locations

### StorageProof
- **Queueing**: `reth/trie/parallel/src/proof.rs:233` - `spawn_storage_proof()`
- **Calculation**: `reth/trie/parallel/src/proof_task.rs:266` - `storage_proof()`
- **Entry Point**: `reth/engine/tree/src/tree/payload_processor/multiproof.rs:514` - `spawn_multiproof()`

### BlindedAccountNode
- **Request**: `reth/trie/parallel/src/proof_task.rs:620` - `ProofTaskTrieNodeProvider::trie_node()`
- **Calculation**: `reth/trie/parallel/src/proof_task.rs:344` - `blinded_account_node()`

### BlindedStorageNode
- **Request**: `reth/trie/parallel/src/proof_task.rs:628` - `ProofTaskTrieNodeProvider::trie_node()`
- **Calculation**: `reth/trie/parallel/src/proof_task.rs:387` - `blinded_storage_node()`

---

## Summary

1. **StorageProof**: Primary proof calculation, 1 per account (deduplicated), calculated upfront
2. **BlindedAccountNode**: Lazy node retrieval, 0-10 per proof, on-demand during account trie walk
3. **BlindedStorageNode**: Lazy node retrieval, 0-20 per proof, on-demand during storage trie walk

**Key Takeaway**: For a block with N transactions touching M unique accounts:
- **StorageProof**: M calculations (not N!)
- **BlindedAccountNode**: ~M × 5 nodes (estimated)
- **BlindedStorageNode**: ~M × 10 nodes (estimated)

This deduplication and parallel execution is what makes the system efficient for large blocks with 10,000+ transactions.

