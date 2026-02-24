// fetch.rs — parallel fetching helpers for Polymarket bot

use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use chrono::{DateTime, Utc, Duration};
use futures::future::join_all;
use chrono_tz::Pacific::Honolulu;
use std::fs;

// ================================================================================
// CONFIG
// Loaded once at startup from config.json, passed around by reference.
// ================================================================================
#[derive(Deserialize)]
pub struct Config {
    pub bot_token: String,
    pub chat_id: String,
    pub hours_window: i64,
    pub pool_max_idle_per_host: usize,
    pub request_timeout_secs: u64,
    pub tag_ids: Vec<String>,
}

impl Config {
    pub fn load(path: &str) -> Config {
        let contents = fs::read_to_string(path)
            .expect("Failed to read config.json");
        serde_json::from_str(&contents)
            .expect("Failed to parse config.json")
    }
}

// ================================================================================
// SHARED CLIENT
// Build once, reuse everywhere. Handles connection pooling automatically.
// ================================================================================
pub fn build_client(config: &Config) -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(config.request_timeout_secs))
        .pool_max_idle_per_host(config.pool_max_idle_per_host)
        .build()
        .expect("Failed to build HTTP client")
}

// ================================================================================
// TIME HELPERS
// ================================================================================
pub fn utc_to_hst(utc_str: &str) -> String {
    let utc: DateTime<Utc> = utc_str.parse().unwrap();
    let hst = utc.with_timezone(&Honolulu);
    hst.format("%B %d, %Y %I:%M %p HST").to_string()
}

pub fn now_and_window(hours: i64) -> (DateTime<Utc>, DateTime<Utc>, String) {
    let now = Utc::now();
    let later = now + Duration::hours(hours);
    let now_str = now.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    (now, later, now_str)
}

// ================================================================================
// PARALLEL TAG FETCHING
// Fires all tag_id requests concurrently instead of one-by-one.
// Returns a flat Vec of all events across all tags.
// ================================================================================
pub async fn fetch_all_tags(client: &Client, tag_ids: &[&str], now_str: &str) -> Vec<Value> {
    let futures: Vec<_> = tag_ids
        .iter()
        .map(|tag_id| {
            let url = format!(
                "https://gamma-api.polymarket.com/events?limit=50&end_date_min={}&closed=false&tag_id={}",
                now_str, tag_id
            );
            let client = client.clone();
            async move {
                match client.get(&url).send().await {
                    Ok(resp) => match resp.json::<Vec<Value>>().await {
                        Ok(events) => events,
                        Err(e) => {
                            eprintln!("Failed to parse events for tag {}: {}", tag_id, e);
                            vec![]
                        }
                    },
                    Err(e) => {
                        eprintln!("Failed to fetch tag {}: {}", tag_id, e);
                        vec![]
                    }
                }
            }
        })
        .collect();

    // All tag requests fire at the same time
    let results = join_all(futures).await;
    results.into_iter().flatten().collect()
}

// ================================================================================
// PARALLEL ORDERBOOK FETCHING
// Collects all token_ids from a market upfront, then fetches all orderbooks
// concurrently in one wave instead of sequentially inside nested loops.
// ================================================================================
#[derive(Debug)]
pub struct OrderbookEntry {
    pub outcome: String,
    pub best_ask: String,
}

pub async fn fetch_orderbooks(
    client: &Client,
    tokens: &[String],   // list of token_id strings
    outcomes: &[String], // parallel list of outcome labels
) -> Vec<OrderbookEntry> {
    let futures: Vec<_> = tokens
        .iter()
        .enumerate()
        .map(|(i, token)| {
            let url = format!("https://clob.polymarket.com/book?token_id={}", token);
            let outcome = outcomes.get(i).cloned().unwrap_or_else(|| "Unknown".to_string());
            let client = client.clone();
            async move {
                let resp = match client.get(&url).send().await {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Orderbook fetch failed for token {}: {}", token, e);
                        return None;
                    }
                };
                let book: Value = match resp.json().await {
                    Ok(v) => v,
                    Err(_) => return None,
                };
                let best_ask = book
                    .get("asks")
                    .and_then(Value::as_array)
                    .and_then(|b| b.last())
                    .and_then(|b| b.get("price"))
                    .and_then(Value::as_str)
                    .map(str::to_string);

                best_ask.map(|ask| OrderbookEntry { outcome, best_ask: ask })
            }
        })
        .collect();

    // All orderbook requests fire at the same time
    join_all(futures).await.into_iter().flatten().collect()
}

// ================================================================================
// EVENT FILTERING
// Pure logic — no I/O. Filters a flat event list down to game events
// ending within the time window. Call after fetch_all_tags().
// ================================================================================
pub fn filter_game_events<'a>(
    events: &'a [Value],
    now: &DateTime<Utc>,
    window_end: &DateTime<Utc>,
) -> Vec<&'a Value> {
    events
        .iter()
        .filter(|event| {
            // Must have a tag marking it as a game (tag id 100639)
            let is_game = event
                .get("tags")
                .and_then(Value::as_array)
                .map(|tags| tags.iter().any(|t| {
                    t.get("id").and_then(Value::as_str) == Some("100639")
                }))
                .unwrap_or(false);

            if !is_game { return false; }

            // Must end within our time window
            let end_date = event
                .get("endDate")
                .and_then(Value::as_str)
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.with_timezone(&Utc));

            matches!(end_date, Some(d) if d > *now && d <= *window_end)
        })
        .collect()
}

// ================================================================================
// EXTRACT MONEYLINE MARKETS
// Pure function — pulls moneyline market token/outcome data from an event.
// Returns Vec of (question, token_ids, outcomes) tuples ready for orderbook fetching.
// ================================================================================
pub fn extract_moneyline_markets(event: &Value) -> Vec<(String, Vec<String>, Vec<String>)> {
    let markets = match event.get("markets").and_then(Value::as_array) {
        Some(m) => m,
        None => return vec![],
    };

    markets
        .iter()
        .filter(|m| m.get("sportsMarketType").and_then(Value::as_str) == Some("moneyline"))
        .filter_map(|market| {
            let question = market.get("question").and_then(Value::as_str)?.to_string();

            let tokens: Vec<String> = serde_json::from_str(
                market.get("clobTokenIds").and_then(Value::as_str).unwrap_or("[]"),
            )
            .unwrap_or_default();

            let outcomes: Vec<String> = serde_json::from_str(
                market.get("outcomes").and_then(Value::as_str).unwrap_or("[]"),
            )
            .unwrap_or_default();

            Some((question, tokens, outcomes))
        })
        .collect()
}

// ================================================================================
// TELEGRAM HELPER
// Thin wrapper so you're not formatting URLs all over main.rs
// ================================================================================
pub async fn tg_send(client: &Client, bot_token: &str, chat_id: &str, text: &str) {
    let url = format!(
        "https://api.telegram.org/bot{}/sendMessage?chat_id={}&text={}",
        bot_token,
        chat_id,
        urlencoding::encode(text)
    );
    if let Err(e) = client.get(&url).send().await {
        eprintln!("Telegram send failed: {}", e);
    }
}

// ================================================================================
// PRINT EVENT
// Shared display function — call after assembling market_entries so both
// main.rs and test binaries produce identical terminal output.
// ================================================================================
pub fn print_event(title: &str, end_date_hst: &str, event_tags: &[String], market_entries: &[serde_json::Value]) {
    println!("EVENT: {} | EndDate: {}", title, end_date_hst);
    println!("==================================================================================");
    println!("  Tags: {}", event_tags.join(", "));
    for entry in market_entries {
        let question = entry.get("question").and_then(|q| q.as_str()).unwrap_or("");
        let sides    = entry.get("sides").and_then(|s| s.as_array()).unwrap();
        println!("  Market: {}", question);
        for side in sides {
            let outcome = side.get("outcome").and_then(|o| o.as_str()).unwrap_or("");
            let ask     = side.get("best_ask").and_then(|a| a.as_str()).unwrap_or("");
            println!("    {} | Ask: {}", outcome, ask);
        }
    }
    println!();
}
