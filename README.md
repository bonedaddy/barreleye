# Barreleye

[![Github Actions](https://img.shields.io/github/actions/workflow/status/barreleye/barreleye/tests.yml)](https://github.com/barreleye/barreleye/actions)
[![Dependency Status](https://deps.rs/repo/github/barreleye/barreleye/status.svg)](https://deps.rs/repo/github/barreleye/barreleye)
[![Crates.io](https://img.shields.io/crates/v/barreleye.svg)](https://crates.io/crates/barreleye)
[![License: MPL 2.0](https://img.shields.io/github/license/barreleye/barreleye)](/LICENSE)
[![Contributions - welcome](https://img.shields.io/badge/contributions-welcome-blue)](/CONTRIBUTING.md "Go to contributions doc")
[![Discord](https://img.shields.io/discord/1026664296861679646?label=discord&logo=discord&color=0abd59)](https://discord.gg/VX8PdWSwNZ)
[![Twitter](https://img.shields.io/twitter/follow/BarreleyeLabs?style=social)](https://twitter.com/BarreleyeLabs)

## What is Barreleye?

Barreleye is an open-source, multi-chain blockchain analytics tool. It's goal is to help answer the following questions:

1. What assets does an address hold?

2. Where did these assets come from?

3. What other wallets might be related?

**Note:** This is an actively developed work-in-progress and not yet ready for production. Use at your own risk

## Try

### Via package manager

```bash
cargo install barreleye
barreleye --help
```

### From source

Requires Rust 1.65.0+:

```bash
git clone https://github.com/barreleye/barreleye
cd barreleye
cargo run -- --help
```

Notes:

- A default config file will be generated on the first run. Optionally, rename `barreleye.sample.toml` to `barreleye.toml`

- [Clickhouse](https://github.com/ClickHouse/ClickHouse) is a requirement for warehouse data storage (default configs point to a locally running server)

- Out of the box Barreleye is configured to use [SQLite](https://www.sqlite.org/) ([MySQL](https://www.mysql.com/) and [PostgreSQL](https://www.postgresql.org/) are also supported)

## How does it work

Barreleye consists of two parts: the indexer and the server. The indexer will connect to specified RPC nodes and continuously process new blocks, and the server will handle requests for processed output. You can decouple the two using CLI params.

Running multiple indexers in parallel is supported, but only one will be active at a time. To start indexing without the server: `cargo run -- --indexer`

To run the server without indexing: `cargo run -- --server`

To run them all together: `cargo run`

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

## MVP Todos

🚧 This project is a work-in-progress and not ready for prod use. Most APIs are under "v0/" and crate versions are "v0.x.x". A quick glance at the current todos:

- [x] Basic indexing for Bitcoin and EVM-based chains
- [x] v0/networks
- [x] v0/addresses
- [x] v0/labels
- [x] v0/keys
- [x] v0/heartbeat
- [x] Basic v0/stats
- [x] Minimal v0/assets
- [ ] Minimal v0/upstream
- [ ] Minimal v0/related

## Random Notes

- For indexing, you might have to set Clickhouse's `max_server_memory_usage_to_ram_ratio` to `2`. [Read more](https://github.com/ClickHouse/ClickHouse/issues/17631)

## Contributing

See [CONTRIBUTING](/CONTRIBUTING.md).

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

## License

Mozilla Public License 2.0 ([LICENSE](LICENSE) or <https://opensource.org/licenses/MPL-2.0>)
