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

## Leverage Research
Let me research what the fastest arb bots are doing on Polymarket specifically.Good research. Here's a comprehensive breakdown of what the top bots are actually doing and what you can implement on the developer side:

---

## Infrastructure Layer

**VPS Location — London or Amsterdam, not New York**
This is the biggest one. Polymarket's main off-chain operations are hosted in AWS eu-west-2 (London), where the CLOB operator handles all order matching, API endpoints, and WebSocket streams. Your session summary says New York — that's suboptimal. Amsterdam is the preferred location, achieving response times of just a few milliseconds to AWS eu-west-2 London, and it also provides regulatory safety compared to Germany.

**Dedicated Polygon RPC Node**
The public `polygon-rpc.com` is shared and slow. Providers like Alchemy, QuickNode, and Infura offer dedicated nodes that reduce query latency for on-chain data like token balances and resolution events. For your sports arb use case, this matters less than for crypto markets since you're not chasing on-chain settlement speed — but it still matters for order confirmation.

---

## Code-Level Optimizations (Rust-Specific)

**Local Orderbook with Incremental WS Updates**
This is what separates fast bots from slow ones. Instead of repeatedly querying the full orderbook via REST, maintain a local orderbook in memory. Use incremental WebSocket updates and monitor sequence numbers to avoid data gaps, and set up automatic reconnection with exponential backoff. This means your arb check fires on the delta update, not after a REST round-trip.

**Zero-Allocation Hot Paths**
There's a community Rust client called `polyfill-rs` specifically built for this. It's a high-performance Polymarket Rust client with latency-optimized data structures and zero-allocation hot paths, designed as a drop-in replacement for `polymarket-rs-client`. Best performance is achieved with connection keep-alive enabled. Worth benchmarking against `rs-clob-client`.

**Heartbeat / Keep-Alive Management**
The official `rs-clob-client` has a built-in heartbeat feature that automatically sends keepalive messages to Polymarket's server — if the client disconnects, all open orders are cancelled. You want this enabled so stale orders don't sit in the book if your WS drops mid-arb.

**Parallel Order Submission**
For a two-leg sports arb (binary market), both orders need to fire simultaneously. You're already using `join_all()` for fetching — apply the same pattern to order submission. Fire both FOK orders concurrently via separate tokio tasks rather than sequentially.

---

## Order Execution Strategy

**FOK is mandatory, not optional**
To avoid partial fills or unfavorable trades, use Fill or Kill (FOK) or Immediate or Cancel (IOC) orders — these ensure trades only go through when your desired spread is available. You already know this, but the implementation detail matters: if leg 1 fills and leg 2 misses, you're holding a directional position, not an arb.

**Spread threshold — use 2.5–3%, not 2%**
Make sure the spreads are above 2.5–3%, factoring in Polymarket's 2% winner fee and any potential gas costs during network congestion. Your current 0.98 breakeven is right at the edge — real bots target a buffer above that.

**Batch cancellations**
Looking at the production market-maker bot configs floating around GitHub, batch cancellations are enabled alongside a cancel/replace interval around 500ms and order lifetime of 3 seconds. If an arb opportunity evaporates, you want to cancel stale limit orders in a single API call, not one by one.

---

## Structural Edge

**RTDS (Real-Time Data Stream)**
Market makers often rely on the RTDS (Real-Time Data Stream) for optimized, low-latency data delivery — this is a more optimized feed than the standard public WebSocket. Worth investigating if Polymarket exposes this to API users.

**Polymarket Relayer for Gasless Transactions**
To simplify transactions during network congestion, use the Polymarket Relayer Client, which enables gasless transactions and eliminates the need to monitor gas prices, reducing order submission failures during volatile markets. Less complexity in your hot path.

---

## The Honest Competitive Picture

Average arbitrage opportunity duration is now 2.7 seconds (down from 12.3 seconds in 2024), with 73% of arbitrage profits captured by sub-100ms execution bots. For the **crypto 15-min markets**, it's basically over — Polymarket introduced dynamic taker fees specifically on those markets, where fees can reach ~3.15% on a 50-cent contract, exceeding typical arbitrage margins and making the strategy unprofitable at scale.

Your sports/soccer cross-market approach is the right call. The logical arb on 3-way soccer markets persists longer because fewer bots are watching correlated markets simultaneously, and the fee environment on longer-dated contracts remains favorable. The edge isn't pure speed — it's that you're monitoring a less-saturated surface.

The highest-leverage thing you can do before writing a single line of execution code: **move the VPS to Amsterdam or London**.