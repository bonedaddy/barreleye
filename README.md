# Barreleye

[![Github Actions](https://img.shields.io/github/actions/workflow/status/barreleye/barreleye/tests.yml)](https://github.com/barreleye/barreleye/actions)
[![dependency status](https://deps.rs/repo/github/barreleye/barreleye/status.svg)](https://deps.rs/repo/github/barreleye/barreleye)
[![License: MPL 2.0](https://img.shields.io/github/license/barreleye/barreleye)](/LICENSE)
[![contributions - welcome](https://img.shields.io/badge/contributions-welcome-blue)](/CONTRIBUTING.md "Go to contributions doc")

[![discord](https://img.shields.io/discord/1026664296861679646?label=discord&logo=discord&color=0abd59)](https://discord.gg/VX8PdWSwNZ)
[![twitter](https://img.shields.io/twitter/follow/BarreleyeLabs?style=social)](https://twitter.com/BarreleyeLabs)

## What is Barreleye?

Barreleye is an open-source, multi-chain blockchain analytics tool. It's goal is to help answer the following questions:

1. What assets does an address hold?

2. Where did these assets come from?

3. What other wallets might be related?

**Note:** This is an actively developed work-in-progress and not yet ready for production 🚧

## Try Barreleye

Requires Rust 1.65.0+:

```bash
git clone https://github.com/barreleye/barreleye
cd barreleye
cargo run
```

Notes:

- A default config file will be generated on the first run. Optionally, rename `barreleye.sample.toml` to `barreleye.toml`

- [Clickhouse](https://github.com/ClickHouse/ClickHouse) is a requirement for warehouse storage. Default config settings point to a local installation

- Out of the box Barreleye is configured to use [SQLite](https://www.sqlite.org/) ([MySQL](https://www.mysql.com/) and [PostgreSQL](https://www.postgresql.org/) are also supported).

## Add networks

A default API key is generated on the first run, so to get it:

```sql
select uuid from api_keys;
```

Add a Bitcoin RPC node:

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
    "rpcEndpoints": ["http://username:password@127.0.0.1:8332"],
    "rps": 100
  }' \
  http://localhost:22775/v0/networks
```

Add an Ethereum RPC node:

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
    "rpcEndpoints": ["http://127.0.0.1:8545"],
    "rps": 100
  }' \
  http://localhost:22775/v0/networks
```

## Notes

- Running multiple indexers in parallel is supported in primary/secondary setup. Nodes will decide between each other which one is the primary.

- For indexing, you might have to set Clickhouse's `max_server_memory_usage_to_ram_ratio` to `2`. [Read more](https://github.com/ClickHouse/ClickHouse/issues/17631).
