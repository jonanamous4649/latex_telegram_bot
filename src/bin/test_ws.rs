// src/bin/test_ws.rs — tests ws.rs against a live token from a real active market
//
// Run with: cargo run --bin test_ws
//
// 1. Loads config.json
// 2. Fetches games using the same fetch.rs functions the main bot uses
// 3. Picks a random game event and extracts its first token ID
// 4. Passes that token to ws::run() to stream live price updates
//
// Kill with Ctrl+C when you've seen enough.

use latex_telegram_bot::{fetch, ws};

#[tokio::main]
async fn main() {
    // ── Load config and build client (same as main bot) ──────────────────────
    let config = fetch::Config::load("config.json");
    let client = fetch::build_client(&config);
    let tag_ids: Vec<&str> = config.tag_ids.iter().map(|s| s.as_str()).collect();

    // ── Fetch games ───────────────────────────────────────────────────────────
    println!("Fetching live games...");
    let (now, window_end, now_str) = fetch::now_and_window(config.hours_window);
    let all_events = fetch::fetch_all_tags(&client, &tag_ids, &now_str).await;

    // Deduplicate
    let mut seen = std::collections::HashSet::new();
    let all_events: Vec<_> = all_events
        .into_iter()
        .filter(|e| {
            let id = e.get("id").and_then(serde_json::Value::as_str).unwrap_or("");
            seen.insert(id.to_string())
        })
        .collect();

    let game_events = fetch::filter_game_events(&all_events, &now, &window_end);
    println!("Found {} game events in window", game_events.len());

    if game_events.is_empty() {
        println!("No active game events found — try widening hours_window in config.json");
        return;
    }

    // ── Pick a random event and extract its first token ───────────────────────
    // Uses a simple time-based seed so it varies each run without needing
    // the 'rand' crate as a dependency.
    let idx = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize) % game_events.len();

    let event = game_events[idx];
    let title = event.get("title").and_then(serde_json::Value::as_str).unwrap_or("Unknown");
    let markets = fetch::extract_moneyline_markets(event);

    if markets.is_empty() {
        println!("Event '{}' has no moneyline markets — try running again", title);
        return;
    }

    // Collect all token IDs across all markets for this event so we monitor
    // the full picture — e.g. Home/Draw/Away for a soccer three-way market
    let tokens: Vec<String> = markets
        .iter()
        .flat_map(|(_, token_ids, _)| token_ids.clone())
        .collect();

    println!("Event:  {}", title);
    println!("Tokens: {} token(s) across {} market(s)", tokens.len(), markets.len());
    for (question, token_ids, outcomes) in &markets {
        println!("  Market: {}", question);
        for (token, outcome) in token_ids.iter().zip(outcomes.iter()) {
            println!("    {} | ...{}", outcome, &token[token.len().saturating_sub(8)..]);
        }
    }
    println!("\nStarting WebSocket stream (Ctrl+C to stop):\n");

    // ── Stream live prices via ws::run() ─────────────────────────────────────
    ws::run(tokens).await;
}
