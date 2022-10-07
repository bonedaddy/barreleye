# Barreleye Insights

![Github Actions](https://github.com/barreleye/barreleye-insights/workflows/tests/badge.svg)
[![dependency status](https://deps.rs/repo/github/barreleye/barreleye-insights/status.svg)](https://deps.rs/repo/github/barreleye/barreleye-insights)

Privacy-friendly blockchain analytics for businesses dealing with digital assets.

This is a work-in-progress and not ready for production.

## Setup

First, fetch the data:

```bash
cargo run scan
```

Then, start the server:

```bash
cargo run server
```

## Run

To get insights for an address:

```
http://localhost:22773/v0/insights?address=0x0
```

```json
{
  "address": "0x0",
  "overview": {
    "netWorth": 0,
    "netWorthCurrency": "USD"
  },
  "compliance": {
    "status": "NO_ISSUES_FOUND"
  }
}
```

