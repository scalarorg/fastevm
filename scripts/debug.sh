export BOOTNODES="enode://d03c963a006cf34b6714986fd68aaaca5c149d15347c97a30d845e08bf761b28f903cd2bed5eb038b8e202dae5c82b8b6c63b0341ef67b7e3998f00b2791a418@34.66.88.11:30303,enode://c873716ffa5de25ff524735cf82ae9525d5903193fbeeba2ebac84912f4843b1d818b4c787bdd5f27c6aad103126d77eebb998fc04acf8a21f5cfbc1d76d2d19@34.66.88.12:30303,enode://3d209fe8d8e2c3d5d4b01bac3c552871680703aa7f1fdfe16ee02ac2bc8943f8c251456505bde0eb92476b55b1962199cf4291443928339b9f89b0327e521e46@34.66.88.13:30303"
export HTTP_PORT=8545
export WS_PORT=8546
export ENGINE_PORT=8551
export P2P_PORT=30303
/usr/local/bin/fastevm-execution node \
    --chain /data/config/genosis.json \
    --datadir /data/execution \
    --engine.always-process-payload-attributes-on-canonical-head \
    --http \
    --http.api eth,net,web3,admin,debug \
    --http.addr 0.0.0.0 \
    --http.port ${HTTP_PORT} \
    --http.corsdomain "*" \
    --ws \
    --ws.api eth,net,web3,admin,debug \
    --ws.addr 0.0.0.0 \
    --ws.port ${WS_PORT} \
    --ws.origins "*" \
    --txpool.max-new-txns 102400 \
    --txpool.max-account-slots 102400 \
    --txpool.max-pending-txns 102400 \
    --txpool.pending-max-count 102400 \
    --txpool.pending-max-size 128 \
    --txpool.max-new-pending-txs-notifications 102400 \
    --txpool.queued-max-count 102400 \
    --txpool.queued-max-size 128 \
    --authrpc.addr 0.0.0.0 \
    --authrpc.port ${ENGINE_PORT} \
    --authrpc.jwtsecret /data/execution/jwt.hex \
    --addr 0.0.0.0 \
    --port ${P2P_PORT} \
    --discovery.addr 0.0.0.0 \
    --discovery.port ${P2P_PORT} \
    --p2p-secret-key /data/execution/p2p/secret.key \
    --bootnodes ${BOOTNODES} \
    --enable-tx-subscription \
    --committed-subdags-per-block 30 \
    --block-build-interval-ms 100 \
    -vvv