#!/bin/sh
set -euo pipefail

generate_secret_key() {
  local key_file=$1
  if [ ! -f "$key_file" ]; then
    #openssl rand -hex 32 | tr -d "\n" > "$key_file"
    dd if=/dev/urandom bs=32 count=1 2>/dev/null | xxd -p -c 64 > "$key_file"
  fi
}

prepare_data_dir() {
  DIR=$1
  mkdir -p ${DIR}
  generate_secret_key ${DIR}/jwt.hex
  # 3) Copy genesis.json from template if needed
  if [ ! -f ${DIR}/genesis.json ]; then
    cp /shared/genesis.template.json ${DIR}/genesis.json
  fi
}

prepare_data_dir /data
for i in 1 2 3 4; do
  prepare_data_dir /data${i}
done
