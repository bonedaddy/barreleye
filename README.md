# Barreleye

![Github Actions](https://github.com/barreleye/barreleye/workflows/tests/badge.svg)
[![dependency status](https://deps.rs/repo/github/barreleye/barreleye/status.svg)](https://deps.rs/repo/github/barreleye/barreleye)

Self-hosted, multi-chain customer analytics & insights for businesses handling digital assets.

This is a work-in-progress and not ready for production 🚧

## Setup (dev)

Requires [Clickhouse](https://github.com/ClickHouse/ClickHouse) running locally.

Database defaults to [SQLite](https://www.sqlite.org/) ([PostgreSQL](https://www.postgresql.org/) and [MySQL](https://www.mysql.com/) are also supported).

Cache defaults to [RocksDB](https://rocksdb.org/).

```bash
cargo run
```

## Add networks

A default API key is created on the first run, so to get it:

```sql
select uuid from api_keys;
```

Add Bitcoin (HTTPS is not supported yet):

```bash
curl -i -X POST \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <API_KEY>" \
  -d '{
    "name": "Bitcoin",
    "tag": "Bitcoin",
    "env": "mainnet",
    "blockchain": "bitcoin",
    "chainId": 0,
    "blockTimeMs": 600000,
    "rpcEndpoints": ["http://username:password@127.0.0.1:8332"]
  }' \
  http://localhost:22775/v0/networks
```

Add Ethereum:

```bash
curl -i -X POST \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <API_KEY>" \
  -d '{
    "name": "Ethereum",
    "tag": "Ethereum",
    "env": "mainnet",
    "blockchain": "evm",
    "chainId": 1,
    "blockTimeMs": 12000,
    "rpcEndpoints": ["http://127.0.0.1:8545"]
  }' \
  http://localhost:22775/v0/networks
```