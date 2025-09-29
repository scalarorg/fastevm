#!/bin/bash

batch_txs() {
    local number_of_txs=${1:-10}
    echo "Running batch transaction test with $number_of_txs txs/sender transactions"
    if [ "$number_of_txs" -eq 1 ]; then
        cargo test --test batch test_batch_transfer_one_transaction  -- --exact --show-output
    elif [ "$number_of_txs" -eq 2 ]; then
        cargo test --test batch test_batch_transfer_two_transaction  -- --exact --show-output
    elif [ "$number_of_txs" -eq 10 ]; then
        cargo test --test batch test_batch_transfer_10  -- --exact --show-output
    elif [ "$number_of_txs" -eq 20 ]; then
        cargo test --test batch test_batch_transfer_20  -- --exact --show-output    
    elif [ "$number_of_txs" -eq 100 ]; then
        cargo test --test batch test_batch_transfer_100  -- --exact --show-output
    fi
    
    check_nonces
}

bulk_txs() {
    cargo test --test batch test_batch_transfer_one_transaction  -- --show-output
    cargo test --test batch test_batch_transfer_two_transactions  -- --show-output
}

check_nonces() {
    cargo test --test batch check_nonces  -- --show-output
}

check_block_hash() {
    cargo test --test cross_nodes test_block_hash_consistency  -- --show-output
}

check_send_txs() {
    for node in 1 2 3 4; do
        echo "Transaction in node $node:"
        cat .local-logs/execution-node$node.log | grep 'Sending batch of'
        cat .local-logs/consensus-node$node.log | grep 'Received transactions'
        echo "--------------------------------"
    done
}

scan_blocks() {
    cargo test test_scan_all_blocks -- --show-output
}

$@