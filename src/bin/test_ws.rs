// src/bin/test_ws.rs — tests ws.rs against a live token from a real active market
//
// Run with: cargo run --bin test_ws

use latex_telegram_bot::{fetch, ws};
use latex_telegram_bot::fetch::print_event;
use serde_json::Value;

#[tokio::main]
async fn main() {
    let config  = fetch::Config::load("config.json");
    let client  = fetch::build_client(&config);
    let tag_ids: Vec<&str> = config.tag_ids.iter().map(|s| s.as_str()).collect();

    println!("Fetching live games...");
    let (now, window_end, now_str) = fetch::now_and_window(config.hours_window);
    let all_events = fetch::fetch_all_tags(&client, &tag_ids, &now_str).await;

    let mut seen = std::collections::HashSet::new();
    let all_events: Vec<_> = all_events
        .into_iter()
        .filter(|e| {
            let id = e.get("id").and_then(Value::as_str).unwrap_or("");
            seen.insert(id.to_string())
        })
        .collect();

    let game_events = fetch::filter_game_events(&all_events, &now, &window_end);
    println!("Found {} game events in window\n", game_events.len());

    if game_events.is_empty() {
        println!("No active game events — try widening hours_window in config.json");
        return;
    }

    let idx = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize) % game_events.len();

    let event   = game_events[idx];
    let markets = fetch::extract_moneyline_markets(event);

    if markets.is_empty() {
        println!("Event has no moneyline markets — try running again");
        return;
    }

    let title        = event.get("title").and_then(Value::as_str).unwrap_or("");
    let end_date_str = event.get("endDate").and_then(Value::as_str).unwrap_or("");
    let end_date_hst = fetch::utc_to_hst(end_date_str);

    let event_tags: Vec<String> = event
        .get("tags").and_then(Value::as_array)
        .map(|arr| arr.iter()
            .filter_map(|t| {
                let id    = t.get("id").and_then(Value::as_str)?;
                let label = t.get("label").and_then(Value::as_str)?;
                Some(format!("{}:{}", id, label))
            })
            .collect())
        .unwrap_or_default();

    // ── Fetch real orderbook prices for the event summary ─────────────────────
    // Collects all tokens and outcomes across all markets, fetches orderbooks
    // in parallel, then maps results back to build market_entries with real asks.
    let all_tokens: Vec<String>  = markets.iter().flat_map(|(_, t, _)| t.clone()).collect();
    let all_outcomes: Vec<String> = markets.iter().flat_map(|(_, _, o)| o.clone()).collect();
    let orderbooks = fetch::fetch_orderbooks(&client, &all_tokens, &all_outcomes).await;

    // Build a lookup from outcome name → best_ask string
    let ask_lookup: std::collections::HashMap<String, String> = orderbooks
        .iter()
        .map(|e| (e.outcome.clone(), e.best_ask.clone()))
        .collect();

    let market_entries: Vec<Value> = markets.iter()
        .map(|(question, _, outcomes)| {
            let sides: Vec<Value> = outcomes.iter()
                .map(|outcome| serde_json::json!({
                    "outcome": outcome,
                    "best_ask": ask_lookup.get(outcome).map(|s| s.as_str()).unwrap_or("—")
                }))
                .collect();
            serde_json::json!({
                "question": question,
                "sides": sides,
                "sports_market_type": "moneyline"
            })
        })
        .collect();

    print_event(title, &end_date_hst, &event_tags, &market_entries);

    // Build (token_id, outcome_name) pairs for ws::run()
    let tokens: Vec<(String, String)> = markets
        .iter()
        .flat_map(|(_, token_ids, outcomes)| {
            token_ids.iter().zip(outcomes.iter())
                .map(|(token, outcome)| (token.clone(), outcome.clone()))
        })
        .collect();

    println!("Monitoring {} token(s) — streaming live prices (Ctrl+C to stop):\n", tokens.len());
    ws::run(tokens).await;
}
