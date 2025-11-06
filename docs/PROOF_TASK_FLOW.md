# Proof Task Flow for Large Blocks (10,000+ Transactions)

## Overview

This document explains how proof tasks are performed for large blocks, with a focus on blocks containing 10,000+ transactions. The system uses a multi-layered parallel architecture to efficiently calculate Merkle proofs.

## Architecture Components

### 1. **PayloadProcessor** (`mod.rs`)
   - Entry point for block processing
   - Detects large blocks and adjusts configuration
   - Spawns all proof-related tasks

### 2. **MultiProofTask** (`multiproof.rs`)
   - Manages multiproof calculation requests
   - Queues and dispatches proof calculations
   - Handles concurrency limits

### 3. **ProofTaskManager** (`proof_task.rs`)
   - Manages database transactions for proof calculations
   - Controls parallelism with `max_concurrency`
   - Reuses transactions to reduce overhead

### 4. **ParallelProof** (`proof.rs`)
   - Calculates multiproofs by spawning storage proofs in parallel
   - Coordinates between account and storage proofs

## Complete Flow Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│ 1. Block Processing Starts (10,000 transactions)                │
│    PayloadProcessor::spawn()                                    │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ 2. Large Block Detection                                         │
│    - Checks: account_prefix_set.len() > 1000 OR                 │
│              storage_prefix_sets total > 5000                     │
│    - Result: is_large_block = true                              │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ 3. Configuration for Large Blocks                                │
│    - Cache: 500k trie nodes, 100k accounts, 500k storage       │
│    - ProofTaskManager concurrency: 2x config (max 512)          │
│    - MultiProofTask concurrency: 75% of proof task concurrency │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ 4. Task Initialization                                           │
│    ┌────────────────────────────────────────────────────────┐   │
│    │ ProofTaskManager (max_concurrency = 256-512)           │   │
│    │ - Manages DB transaction pool                           │   │
│    │ - Spawned in blocking thread                            │   │
│    └────────────────────────────────────────────────────────┘   │
│    ┌────────────────────────────────────────────────────────┐   │
│    │ MultiProofTask (max_concurrency = 192-384)            │   │
│    │ - Manages multiproof requests                           │   │
│    │ - Spawned in blocking thread                            │   │
│    └────────────────────────────────────────────────────────┘   │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ 5. Transaction Execution                                         │
│    - Each transaction executed and state changes tracked         │
│    - Proof targets accumulated as transactions are processed     │
│    - Prefetch requests sent to MultiProofTask                    │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ 6. MultiProofTask Receives Proof Request                         │
│    - Chunks proof targets (dynamic chunk size)                   │
│    - Checks if max_concurrent multiproofs are inflight          │
│    - Queues if at limit, spawns otherwise                        │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ 7. ParallelProof::decoded_multiproof() Called                   │
│    - Input: MultiProofTargets (account -> storage slots map)    │
│    - Example: 1000 accounts, 5000 storage slots total            │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ 8. Storage Proof Spawning (CRITICAL PARALLELIZATION POINT)       │
│                                                                   │
│    for each (hashed_address, prefix_set) in storage_targets:     │
│        ┌─────────────────────────────────────────────────────┐   │
│        │ ParallelProof::spawn_storage_proof()                │   │
│        │ - Creates StorageProofInput                          │   │
│        │ - Queues task to ProofTaskManager                    │   │
│        │ - Returns receiver channel                           │   │
│        └──────────────────┬──────────────────────────────────┘   │
│                          │                                        │
│                          ▼                                        │
│        ┌─────────────────────────────────────────────────────┐   │
│        │ ProofTaskManager::queue_proof_task()                │   │
│        │ - Adds to pending_tasks queue                        │   │
│        │ - ProofTaskManager::run() picks it up                │   │
│        └─────────────────────────────────────────────────────┘   │
│                                                                   │
│    Result: All storage proofs queued immediately (parallel)      │
│    - 1000 storage proofs queued in ~1ms                          │
│    - No blocking, all queued before any calculation starts       │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ 9. ProofTaskManager::run() Loop (Background Thread)              │
│                                                                   │
│    Loop:                                                          │
│      ┌──────────────────────────────────────────────────────┐    │
│      │ 1. Receive QueueTask message                        │    │
│      │    - Add to pending_tasks queue                      │    │
│      │                                                       │    │
│      │ 2. try_spawn_next()                                  │    │
│      │    - Get available ProofTaskTx (or create new)       │    │
│      │    - If at max_concurrency, wait for tx to return    │    │
│      │    - Spawn blocking task: storage_proof()             │    │
│      │                                                       │    │
│      │ 3. Receive Transaction message                       │    │
│      │    - ProofTaskTx completed, return to pool           │    │
│      │    - try_spawn_next() to process queued task         │    │
│      └──────────────────────────────────────────────────────┘    │
│                                                                   │
│    Concurrency Control:                                          │
│    - Up to max_concurrency (256-512) tasks running               │
│    - Tasks beyond limit wait in pending_tasks queue             │
│    - As tasks complete, new ones are spawned                     │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ 10. Storage Proof Calculation (ProofTaskTx::storage_proof)       │
│                                                                   │
│     Each ProofTaskTx (in blocking thread):                       │
│     ┌──────────────────────────────────────────────────────┐    │
│     │ 1. Create cursor factories (reused transaction)      │    │
│     │ 2. Calculate storage proof:                           │    │
│     │    StorageProof::new_hashed()                         │    │
│     │      .storage_multiproof(target_slots)                │    │
│     │ 3. Decode proof nodes                                 │    │
│     │ 4. Send result back via channel                        │    │
│     │ 5. Return ProofTaskTx to pool                         │    │
│     └──────────────────────────────────────────────────────┘    │
│                                                                   │
│     Parallel Execution:                                          │
│     - 256-512 proofs calculated simultaneously                  │
│     - Each uses its own DB transaction (read-only)              │
│     - Shared cache reduces disk I/O                              │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ 11. Account Trie Walking (ParallelProof continues)              │
│                                                                   │
│     While storage proofs are being calculated:                   │
│     ┌──────────────────────────────────────────────────────┐    │
│     │ - Walk account trie using TrieWalker                  │    │
│     │ - For each account leaf:                              │    │
│     │   * Check if storage proof receiver exists           │    │
│     │   * recv() to get storage proof (blocking wait)       │    │
│     │   * If no receiver, calculate fallback proof          │    │
│     │   * Build account RLP with storage root               │    │
│     │   * Add to HashBuilder                                │    │
│     └──────────────────────────────────────────────────────┘    │
│                                                                   │
│     This ensures storage proofs are ready when needed            │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ 12. Multiproof Assembly Complete                                │
│     - All account nodes collected                               │
│     - All storage proofs collected                              │
│     - DecodedMultiProof returned                                │
└─────────────────────────────────────────────────────────────────┘
```

## Key Performance Characteristics

### For Large Blocks (10,000 transactions)

1. **Block Detection**: Automatic detection based on prefix set sizes
   - `account_prefix_set.len() > 1000` OR
   - `storage_prefix_sets total > 5000`

2. **Concurrency Settings**:
   ```
   Default max_proof_task_concurrency: ~128-256
   Large block max_proof_task_concurrency: 256-512 (doubled, capped at 512)
   Large block max_multi_proof_task_concurrency: 192-384 (75% of proof task)
   ```

3. **Cache Sizes** (Large Blocks):
   ```
   Trie nodes: 500,000
   Accounts: 100,000
   Storage slots: 500,000
   ```

4. **Parallelization Strategy**:
   - **Phase 1**: All storage proofs queued immediately (non-blocking)
   - **Phase 2**: ProofTaskManager processes up to 256-512 proofs concurrently
   - **Phase 3**: Account trie walker waits for storage proofs as needed
   - **Result**: Maximum parallelism with minimal blocking

## Example: 10,000 Transaction Block

### Scenario
- 10,000 transactions
- 1,500 unique accounts touched
- 8,000 storage slots modified
- Average: ~5.3 storage slots per account

### Flow Timeline

```
T+0ms:    Block processing starts
T+1ms:    Large block detected, configuration applied
T+2ms:    ProofTaskManager and MultiProofTask spawned
T+5ms:    First proof requests arrive at MultiProofTask
T+10ms:   All 1,500 storage proofs queued to ProofTaskManager
T+10ms:   ProofTaskManager starts processing (up to 512 concurrent)
T+50ms:   First batch of storage proofs complete
T+100ms:  Account trie walker starts, waiting on storage proofs
T+200ms:  ~50% of storage proofs complete
T+400ms:  ~90% of storage proofs complete
T+500ms:  All storage proofs complete, account trie walker finishes
T+510ms:  Multiproof assembly complete
```

### Bottlenecks

1. **Database I/O**: Each proof task needs to read from DB
   - Mitigation: Large shared cache (500k nodes)
   - Mitigation: Read-only transactions
   - Mitigation: Cached cursor factories

2. **Concurrency Limit**: Only 256-512 proofs can run simultaneously
   - Mitigation: Automatically doubled for large blocks
   - Mitigation: Transaction pool reuses connections

3. **Queue Time**: If >512 proofs, some wait in queue
   - Mitigation: Proofs are queued upfront, so queue is processed efficiently
   - Mitigation: As proofs complete, new ones start immediately

## Optimization Opportunities

### Current Optimizations
- ✅ Large block detection and automatic configuration
- ✅ Doubled concurrency for large blocks
- ✅ Large cache sizes for large blocks
- ✅ Transaction reuse via ProofTaskManager
- ✅ All storage proofs queued upfront (max parallelism)

### Potential Further Optimizations

1. **Dynamic Concurrency**: Adjust based on actual proof calculation time
2. **Batch Processing**: Group related storage proofs together
3. **Prefetching**: Start account trie walk earlier if possible
4. **Cache Warming**: Pre-populate cache with likely-needed nodes

## Monitoring

Key metrics to watch:
- `trie::parallel_proof`: Multiproof generation timing
- `trie::proof_task`: Individual proof task timing
- `engine::root`: Overall proof calculation timing
- Pending queue length in ProofTaskManager
- Inflight multiproof count in MultiproofManager

## Configuration

To adjust performance for large blocks, modify:
- `max_proof_task_concurrency()` in TreeConfig
- Large block detection thresholds (lines 191-192 in `mod.rs`)
- Cache sizes in `ProofTaskCtx::for_large_block()`

## Key Insights for Large Blocks

### 1. **Three-Level Parallelism**

```
Level 1: MultiProofTask chunks (e.g., 100 accounts per chunk)
  ↓
Level 2: ParallelProof spawns all storage proofs upfront
  ↓
Level 3: ProofTaskManager executes up to 512 proofs concurrently
```

### 2. **Chunking Strategy**

For 10,000 transactions with 1,500 accounts:
- Chunk size: 100 accounts (VERY_LARGE threshold)
- Number of chunks: ~15 chunks
- Each chunk processed by separate ParallelProof instance
- Each ParallelProof spawns ~100 storage proofs in parallel

### 3. **Critical Path**

The account trie walker is the critical path:
- It waits for storage proofs as it encounters account leaves
- Storage proofs are calculated in parallel (up to 512 concurrent)
- If storage proof ready: use it immediately
- If not ready: blocking wait (usually <100ms)
- If missed: fallback calculation (rare, but slower)

### 4. **Why This Works Well**

1. **Non-blocking Queue**: All storage proofs queued immediately
2. **High Concurrency**: 256-512 proofs can run simultaneously
3. **Transaction Reuse**: DB transactions returned to pool, reused
4. **Large Cache**: 500k nodes cached, reducing disk I/O
5. **Lazy Waiting**: Account walker only waits when needed

### 5. **Performance Scaling**

For blocks with N accounts:
- **Small (N < 100)**: Chunk size 10, normal concurrency
- **Medium (100 < N < 1000)**: Chunk size 50, normal concurrency
- **Large (N > 1000)**: Chunk size 100, 2x concurrency, large cache

### 6. **Queue Management**

```
ProofTaskManager maintains:
- pending_tasks: Queue of tasks waiting for available transaction
- proof_task_txs: Pool of available ProofTaskTx instances
- total_transactions: Current number of active transactions

When task completes:
1. ProofTaskTx returned to pool
2. try_spawn_next() called immediately
3. Next pending task starts without delay
```

### 7. **Memory Considerations**

For large blocks:
- Shared cache: ~500MB (500k nodes × ~1KB/node)
- Transaction pool: 256-512 transactions × ~1MB each = ~256-512MB
- Total: ~750MB-1GB for proof calculation infrastructure

### 8. **Debugging Tips**

If proofs are slow:
1. Check `max_proof_task_concurrency` - should be 256-512 for large blocks
2. Monitor queue length - should stay low (<100)
3. Check cache hit rate - should be >80%
4. Verify large block detection is working (check logs)
5. Monitor DB I/O - should be reduced by cache

## Code Locations

Key files for understanding the flow:
- `reth/engine/tree/src/tree/payload_processor/mod.rs` (lines 166-265): Initialization
- `reth/engine/tree/src/tree/payload_processor/multiproof.rs` (lines 514-584): Multiproof spawning
- `reth/trie/parallel/src/proof.rs` (lines 177-371): Parallel proof calculation
- `reth/trie/parallel/src/proof_task.rs` (lines 185-217): Task manager loop

