# Polymarket Arb Bot — Session Summary

## Project Overview
Building a Polymarket arbitrage bot in Rust that monitors sports betting markets for arbitrage opportunities. Currently running and working in dev mode.

## Current File Structure
```
my_project/
├── Cargo.toml
├── config.json      
└── src/
    ├── main.rs
    └── fetch.rs
```

## What Is Built And Working
- `fetch.rs` — parallel fetching module containing:
  - `Config` struct with `#[derive(Deserialize)]` loaded from `config.json`
  - `build_client()` — single shared reqwest client with connection pooling
  - `fetch_all_tags()` — fetches all 25 Polymarket tag IDs in parallel via `join_all()`
  - `fetch_orderbooks()` — fetches all CLOB orderbook prices in parallel
  - `filter_game_events()` — pure filter, no I/O, filters events in 8hr time window
  - `extract_moneyline_markets()` — extracts moneyline market token/outcome data
  - `tg_send()` — Telegram message helper using urlencoding
  - `utc_to_hst()` and `now_and_window()` — time helpers

- `main.rs` — main bot loop:
  - Loads config from `config.json`
  - Polls Telegram for "fetch games" command
  - Fetches all tags in parallel
  - Deduplicates events by ID (same event can appear under multiple tags)
  - Filters to game events ending within 8hr window
  - Groups markets by event using `EventJob` struct (handles soccer 3-way markets correctly)
  - Flattens to `FlatJob` list for parallel orderbook fetching
  - Assembles final JSON output grouped correctly per event
  - Saves to `events/polymarket_btc_events.json`
  - Sends Telegram confirmation

## Cargo.toml Dependencies
```toml
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1.0"
chrono = "0.4"
chrono-tz = "0.8"
tungstenite = { version = "0.20", features = ["native-tls"] }
tokio = { version = "1", features = ["full"] }
futures = "0.3"
urlencoding = "2"
```

## config.json Parameters
```json
{
    "bot_token": "...",
    "chat_id": "...",
    "hours_window": 8,
    "pool_max_idle_per_host": 20,
    "request_timeout_secs": 10,
    "tag_ids": ["100149", "101178", ... 25 total]
}
```

## Sample Terminal Output
```
Fetching 25 tags in parallel...
Got 488 total events across all tags
438 unique events after dedup
11 game events in window
Fetching orderbooks for 13 markets in parallel...
EVENT: Brøndby IF vs. Sønderjyske Fodbold | EndDate: February 23, 2026 08:00 AM HST
  Tags: 1:Sports, 102652:Denmark Superliga, 100639:Games, 100350:Soccer
  Market: Will Brøndby IF win on 2026-02-23?
    Yes | Ask: 0.5
    No | Ask: 0.51
```

## Arb Strategy Research
- Pure same-market arb windows are now ~2.7 seconds average, 73% captured by sub-100ms bots
- **Breakeven threshold is 0.98** (not 1.00) after Polymarket's 2% fee on winnings
- Most realistic opportunity: **cross-market logical arb** on correlated markets
  - Example: soccer three-way markets (Home Win + Draw + Away Win must sum to 1)
  - These persist longer because fewer bots monitor correlated markets simultaneously
- Polymarket CLOB runs on **AWS eu-west-2 London**, best VPS location is New York
- **QuantVPS** (quantvps.com) offers Polymarket-optimized servers in NY and Amsterdam

## Next Steps To Build
The current code only READS data — no order execution exists yet. The full architecture needs:

1. **WebSocket monitoring (next priority)**
   - Polymarket WebSocket: `wss://ws-subscriptions-clob.polymarket.com/ws/market`
   - Replace REST orderbook polling with real-time price push updates
   - Subscribe to specific token IDs discovered via current REST fetch
   - Crate to use: `tokio-tungstenite` (async, fits existing tokio architecture)

2. **Arb detection logic**
   - Run sum check on every incoming WebSocket price update
   - For binary markets: side1_ask + side2_ask < 0.98
   - For soccer three-way: yes_ask1 + yes_ask2 + yes_ask3 < 0.98
   - Flag opportunity with token IDs, current prices, expected profit

3. **Order execution**
   - Polymarket has an official Rust CLOB client: `rs-clob-client` on their GitHub
   - Supports FOK (Fill or Kill) orders — both sides execute or neither does
   - Requires wallet private key for signing orders
   - Requires funded Polymarket account
   - Polygon gas fees ~$0.01-0.05 per transaction (negligible)

## Architecture Plan
```
REST (existing) → discover upcoming games, get token IDs
WebSocket (next) → monitor those token IDs for live price changes  
Arb detector (next) → check sum on every price update, flag if < 0.98
Order executor (future) → fire FOK orders via rs-clob-client
Telegram (existing) → notifications when arb found or orders placed
```

## Key Technical Notes
- Already using `tokio` async runtime — `tokio-tungstenite` fits naturally
- `tungstenite` already in Cargo.toml as placeholder for WebSocket phase
- The REST fetch loop and WebSocket listener would run concurrently as separate tokio tasks
- Bot is running on Mac locally, eventual deployment target is New York VPS for low latency
- Polymarket US is now CFTC regulated and fully accessible from Hawaii without VPN
