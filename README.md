# Barreleye

![Github Actions](https://github.com/barreleye/barreleye/workflows/tests/badge.svg)
[![dependency status](https://deps.rs/repo/github/barreleye/barreleye/status.svg)](https://deps.rs/repo/github/barreleye/barreleye)

Self-hosted, multi-chain customer analytics & insights for businesses handling digital assets.

This is a work-in-progress and not ready for production 🚧

## Setup (dev)

Requires [Anvil](https://book.getfoundry.sh/anvil/) and [Clickhouse](https://github.com/ClickHouse/ClickHouse) running locally (defaults to [SQLite](https://www.sqlite.org/), but [PostgreSQL](https://www.postgresql.org/) and [MySQL](https://www.mysql.com/) are supported):

```bash
cargo run server -w --env localhost
```