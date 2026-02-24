// main.rs — refactored to use parallel fetching from fetch.rs
//
// Add to Cargo.toml:
// tokio = { version = "1", features = ["full"] }
// serde = { version = "1", features = ["derive"] }

mod fetch;

use fetch::{
    build_client, fetch_all_tags, fetch_orderbooks,
    filter_game_events, extract_moneyline_markets,
    now_and_window, utc_to_hst, tg_send, print_event,
    Config,
};
use serde_json::Value;
use std::fs::{self, write};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    // ── Load config ──────────────────────────────────────────────────────────────
    let config = Config::load("config.json");

    // Build ONE client — shared across all requests for the lifetime of the bot
    let client = build_client(&config);
    let mut offset: i64 = 0;

    // Convert Vec<String> from config into Vec<&str> for fetch_all_tags
    let tag_ids: Vec<&str> = config.tag_ids.iter().map(|s| s.as_str()).collect();

    // ── Stdin command channel ────────────────────────────────────────────────────
    // Spawns a background task that reads lines from stdin and forwards any
    // recognised commands into the main loop via a channel — non-blocking.
    let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(8);
    tokio::task::spawn_blocking(move || {
        println!("Terminal ready — type 'fetch games' and press Enter");
        use std::io::BufRead;
        let stdin = std::io::stdin();
        let mut lines = stdin.lock().lines();
        loop {
            match lines.next() {
                Some(Ok(line)) => {
                    let cmd = line.trim().to_string();
                    if !cmd.is_empty() {
                        let _ = stdin_tx.blocking_send(cmd);
                    }
                }
                _ => break, // stdin closed / EOF
            }
        }
    });

    loop {
        // ── Build the Telegram future (not awaited yet) ──────────────────────────
        // timeout=5 keeps the poll short so select! can react to terminal input
        // within a few seconds even if Telegram has nothing to say.
        let tg_url = format!(
            "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=5",
            config.bot_token, offset
        );
        let tg_future = client.get(&tg_url).send();

        // ── Race Telegram vs terminal input ──────────────────────────────────────
        // select! waits for whichever future completes first.
        // If a terminal command arrives while Telegram is still waiting, it wins
        // immediately — no need to wait for the poll to finish.
        tokio::select! {
            // ── Telegram arm ────────────────────────────────────────────────────
            result = tg_future => {
                let body = match result {
                    Ok(r) => r.text().await.unwrap_or_default(),
                    Err(_) => { tokio::time::sleep(std::time::Duration::from_secs(2)).await; continue; }
                };

                let json: Value = serde_json::from_str(&body).unwrap_or_default();
                let updates = match json.get("result").and_then(Value::as_array) {
                    Some(u) => u.clone(),
                    None => { continue; }
                };

                for update in &updates {
                    let update_id = update.get("update_id").and_then(Value::as_i64).unwrap_or(0);
                    offset = update_id + 1;

                    let text = update
                        .get("message")
                        .and_then(|m| m.get("text"))
                        .and_then(Value::as_str)
                        .unwrap_or("");

                    if text != "fetch games" { continue; }

                    println!("Received 'fetch games' command (Telegram)");
                    tg_send(&client, &config.bot_token, &config.chat_id, "Received 'fetch games' command, running...").await;
                    run_fetch(&client, &config, &tag_ids).await;
                }
            }

            // ── Terminal arm ─────────────────────────────────────────────────────
            // stdin_rx.recv() is async — it suspends until a command arrives,
            // which lets select! race it properly against the Telegram future.
            Some(cmd) = stdin_rx.recv() => {
                if cmd == "fetch games" {
                    println!("Received 'fetch games' command (terminal)");
                    // tg_send(&client, &config.bot_token, &config.chat_id, "Received 'fetch games' command, running...").await;
                    run_fetch(&client, &config, &tag_ids).await;
                } else {
                    println!("Unknown command: '{}' — try 'fetch games'", cmd);
                }
            }
        }
    }
}

// ================================================================================
// RUN FETCH
// Extracted from the main loop so both Telegram and terminal commands
// can trigger it without duplicating the logic.
// ================================================================================
async fn run_fetch(client: &reqwest::Client, config: &Config, tag_ids: &[&str]) {
    // ── Time window ──────────────────────────────────────────────────────────
    let (now, window_end, now_str) = now_and_window(config.hours_window);

    // ── 1. Fetch all tags IN PARALLEL ────────────────────────────────────────
    println!("Fetching {} tags in parallel...", tag_ids.len());
    let all_events = fetch_all_tags(client, tag_ids, &now_str).await;
    println!("Got {} total events across all tags", all_events.len());

    // ── 2. Deduplicate by event id ────────────────────────────────────────────
    let mut seen_ids = std::collections::HashSet::new();
    let all_events: Vec<Value> = all_events
        .into_iter()
        .filter(|e| {
            let id = e.get("id").and_then(Value::as_str).unwrap_or("");
            seen_ids.insert(id.to_string())
        })
        .collect();
    println!("{} unique events after dedup", all_events.len());

    // ── 3. Filter to game events in the time window ───────────────────────────
    let game_events = filter_game_events(&all_events, &now, &window_end);
    println!("{} game events in window", game_events.len());

    // ── 4. Build jobs grouped by event ───────────────────────────────────────
    struct MarketJob {
        question: String,
        tokens: Vec<String>,
        outcomes: Vec<String>,
    }
    struct EventJob<'a> {
        event: &'a Value,
        markets: Vec<MarketJob>,
    }

    let event_jobs: Vec<EventJob> = game_events
        .iter()
        .map(|event| {
            let markets = extract_moneyline_markets(event)
                .into_iter()
                .map(|(question, tokens, outcomes)| MarketJob { question, tokens, outcomes })
                .collect();
            EventJob { event, markets }
        })
        .collect();

    // ── 5. Flatten and fetch all orderbooks in one parallel wave ──────────────
    struct FlatJob<'a> {
        event_idx: usize,
        market: &'a MarketJob,
    }

    let flat_jobs: Vec<FlatJob> = event_jobs
        .iter()
        .enumerate()
        .flat_map(|(i, ej)| ej.markets.iter().map(move |m| FlatJob { event_idx: i, market: m }))
        .collect();

    println!("Fetching orderbooks for {} markets in parallel...\n", flat_jobs.len());
    let all_orderbooks = futures::future::join_all(
        flat_jobs.iter().map(|fj| fetch_orderbooks(client, &fj.market.tokens, &fj.market.outcomes))
    ).await;

    // ── 6. Assemble JSON output ───────────────────────────────────────────────
    let mut filtered: Vec<Value> = Vec::new();

    for (event_idx, event_job) in event_jobs.iter().enumerate() {
        let event = event_job.event;

        let id           = event.get("id").and_then(Value::as_str).unwrap_or("");
        let title        = event.get("title").and_then(Value::as_str).unwrap_or("");
        let slug         = event.get("slug").and_then(Value::as_str).unwrap_or("");
        let end_date_str = event.get("endDate").and_then(Value::as_str).unwrap_or("");
        let end_date_hst = utc_to_hst(end_date_str);

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

        let mut market_entries: Vec<Value> = Vec::new();

        for (fj, sides) in flat_jobs.iter().zip(all_orderbooks.iter()) {
            if fj.event_idx != event_idx { continue; }
            if sides.len() < 2 { continue; }

            let side_entries: Vec<Value> = sides.iter()
                .map(|s| serde_json::json!({ "outcome": s.outcome, "best_ask": s.best_ask }))
                .collect();

            market_entries.push(serde_json::json!({
                "question": fj.market.question,
                "sides": side_entries,
                "sports_market_type": "moneyline",
            }));
        }

        if market_entries.is_empty() { continue; }

        print_event(title, &end_date_hst, &event_tags, &market_entries);

        filtered.push(serde_json::json!({
            "id": id,
            "tag_id": event_tags,
            "title": title,
            "slug": slug,
            "endDateHST": end_date_hst,
            "market_entries": market_entries
        }));
    }

    // ── 7. Save ───────────────────────────────────────────────────────────────
    let result = serde_json::to_string_pretty(&filtered).unwrap();
    fs::create_dir_all("events").unwrap();
    write("events/polymarket_btc_events.json", result).unwrap();

    // tg_send(client, &config.bot_token, &config.chat_id, ".json file updated!").await;
}
