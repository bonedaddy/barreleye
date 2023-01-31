# Barreleye

[![Status beta](https://img.shields.io/badge/status-beta-ff69b4.svg?style=flat-square)](https://github.com/barreleye/barreleye)
[![Contributions](https://img.shields.io/badge/contributions-welcome-ff69b4?style=flat-square)](/CONTRIBUTING.md "Go to contributions doc")
[![Crates.io](https://img.shields.io/crates/v/barreleye?color=brightgreen&style=flat-square)](https://crates.io/crates/barreleye)
[![Github Actions](https://img.shields.io/github/actions/workflow/status/barreleye/barreleye/tests.yml?style=flat-square)](https://github.com/barreleye/barreleye/actions)
[![Dependency Status](https://deps.rs/repo/github/barreleye/barreleye/status.svg?style=flat-square)](https://deps.rs/repo/github/barreleye/barreleye)
[![License](https://img.shields.io/github/license/barreleye/barreleye?color=orange&style=flat-square)](/LICENSE)
[![Downloads](https://img.shields.io/crates/d/barreleye?color=blue&style=flat-square)](https://crates.io/crates/barreleye)
![Activity](https://img.shields.io/github/commit-activity/m/barreleye/barreleye?style=flat-square)
[![Discord](https://img.shields.io/discord/1026664296861679646?style=flat-square&color=blue)](https://discord.gg/VX8PdWSwNZ)
[![Twitter](https://img.shields.io/twitter/follow/barreleyelabs?color=blue&style=flat-square)](https://twitter.com/BarreleyeLabs)

## What is Barreleye?

Barreleye is an open-source, multi-chain blockchain analytics tool. It's goal is to help answer the following questions:

1. What assets does an address hold?
1. Where did these assets come from?
1. What other wallets might be related?

**Note:** This is an actively developed work-in-progress and not yet ready for production. Use at your own risk ⚠️

## Try

Barreleye requires [Clickhouse](https://github.com/ClickHouse/ClickHouse) 22.8+ to run (default configs point to a locally running server):

### Via package manager

```bash
cargo install barreleye
barreleye --warehouse=http://localhost:8123/database_name
```

### From source

Requires Rust 1.65.0+:

```bash
git clone https://github.com/barreleye/barreleye
cd barreleye
cargo run -- --warehouse=http://localhost:8123/database_name
```

Notes:

- Use `barreleye --help` to see all options
- Default RDBMS is configured to use [SQLite](https://www.sqlite.org/) ([MySQL](https://www.mysql.com/) and [PostgreSQL](https://www.postgresql.org/) are also supported)
- Clickhouse 22.8+ is required because it supports `allow_experimental_lightweight_delete` for MergeTree table engine family.

## Basics

Barreleye consists of two parts: the indexer and the server. The indexer connects to specified RPC nodes to process blocks, and the server handles management and analytics requests.

**Note:** Indexing continuously processes data from the genesis block. Make sure your RPC node can handle the amount of requests.

To start just the indexer, without the server: `cargo run -- --only-indexer`. Note that only one indexer is active at a time.

To run only the HTTP server: `cargo run -- --only-http`

To run them all together: `cargo run`

## Add networks

Two default API keys are generated on the first run (one admin key; one regular key for analytics requests). You can get them by running this in your RDBMS:

```sql
select uuid from api_keys where is_admin=true; -- to get admin key
select uuid from api_keys where is_admin=false; -- to get regular key
```

Add a Bitcoin RPC node:

```bash
curl -X POST \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <ADMIN_API_KEY>" \
  -d '{
    "name": "Bitcoin",
    "env": "mainnet",
    "blockchain": "bitcoin",
    "chainId": 0,
    "blockTimeMs": 600000,
    "rpcEndpoints": ["http://username:password@127.0.0.1:8332"],
    "rps": 100
  }' \
  http://localhost:22775/v0/networks
```

Add an EVM-based RPC node:

```bash
curl -X POST \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <ADMIN_API_KEY>" \
  -d '{
    "name": "Ethereum",
    "env": "mainnet",
    "blockchain": "evm",
    "chainId": 1,
    "blockTimeMs": 12000,
    "rpcEndpoints": ["http://127.0.0.1:8545"],
    "rps": 100
  }' \
  http://localhost:22775/v0/networks
```

⏳ Indexing will take a while. To monitor progress:

```bash
curl -X GET \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <ADMIN_API_KEY>" \
  http://localhost:22775/v0/stats
```

## Analytics

To get networks, assets, labels, etc:

```bash
curl -X GET \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <API_KEY>" \
  http://localhost:22775/v0/info?address=<BLOCKCHAIN_ADDRESS>
```

To find connected labeled addresses that might have funded the requested address through multiple hops:

```bash
curl -X GET \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <API_KEY>" \
  http://localhost:22775/v0/upstream?address=<BLOCKCHAIN_ADDRESS>
```

## Random Notes

- Be aware of your RPC node limits. Indexer makes a significant amount of RPC calls to index historical and new blocks.
- For indexing, you might have to set Clickhouse's `max_server_memory_usage_to_ram_ratio` to `2` ([read more](https://github.com/ClickHouse/ClickHouse/issues/17631))
- Warehouse's `experimental_relations` table (along with modules) is not accurate and should not be relied on right now

## Get Involved

To stay in touch with Barreleye:

- Star this repo ★
- Follow on [Twitter](https://twitter.com/BarreleyeLabs)
- Join on [Discord](https://discord.gg/VX8PdWSwNZ)
- [Contribute](/CONTRIBUTING.md) -- pull requests are welcome (for major changes, please open an issue first to discuss what you would like to change)

## License

Source code for Barreleye is variously licensed under a number of different licenses. A copy of each license can be found in [each repository](https://github.com/barreleye).

- Libraries and SDKs, each located in its own distinct repository, are released under either the [Apache License 2.0](https://opensource.org/licenses/Apache-2.0) or [MIT License](https://opensource.org/licenses/MIT).
- Core code for Barreleye, located in [this repository](https://github.com/barreleye/barreleye), is released under the [GNU Affero General Public License 3.0](/LICENSE).
