// ws.rs — Polymarket CLOB WebSocket price monitor
//
// Connects to Polymarket's real-time order book stream and prints live
// price updates to the terminal. Runs as a background tokio task.
//
// Polymarket WebSocket docs:
// wss://ws-subscriptions-clob.polymarket.com/ws/market
// Subscribe by sending a JSON message with the token IDs you want to watch.

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message};

const WS_URL: &str = "wss://ws-subscriptions-clob.polymarket.com/ws/market";

// ── Entry point ───────────────────────────────────────────────────────────────
// Spawn this with: tokio::spawn(ws::run(token_ids));
// token_ids comes from the REST fetch phase — the list of CLOB token IDs
// for the markets you want to monitor.
pub async fn run(token_ids: Vec<String>) {
    loop {
        println!("[WS] Connecting to Polymarket...");

        match connect_and_stream(&token_ids).await {
            Ok(_) => println!("[WS] Stream ended, reconnecting..."),
            Err(e) => println!("[WS] Connection error: {e}, reconnecting in 5s..."),
        }

        // Brief pause before reconnecting to avoid hammering the server
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

// ── Connect, subscribe, and stream messages ───────────────────────────────────
// Separated from run() so errors bubble up cleanly and the retry loop stays tidy.
async fn connect_and_stream(token_ids: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // Open the WebSocket connection
    let (mut ws, _) = connect_async(WS_URL).await?;
    println!("[WS] Connected — subscribing to {} tokens", token_ids.len());

    // Send the subscription message
    // Polymarket expects: { "assets_ids": [...], "type": "market" }
    let sub_msg = json!({
        "assets_ids": token_ids,
        "type": "market"
    });
    ws.send(Message::Text(sub_msg.to_string())).await?;
    println!("[WS] Subscribed. Waiting for price updates...\n");

    // ── Message loop ──────────────────────────────────────────────────────────
    while let Some(msg) = ws.next().await {
        match msg? {
            Message::Text(text) => handle_message(&text),
            Message::Ping(data) => {
                // Respond to pings to keep the connection alive
                ws.send(Message::Pong(data)).await?;
            }
            Message::Close(_) => {
                println!("[WS] Server closed connection");
                break;
            }
            _ => {} // ignore binary frames and other message types
        }
    }

    Ok(())
}

// ── Parse and print a single price update ────────────────────────────────────
// Polymarket sends an array of order book events per message.
// Each event has an asset_id (token ID) and a list of changes.
fn handle_message(text: &str) {
    // Parse the raw JSON — if it fails just print the raw text so we can debug
    let events: Vec<Value> = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => {
            println!("[WS] Raw: {text}");
            return;
        }
    };

    for event in &events {
        let asset_id = event
            .get("asset_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");

        let event_type = event
            .get("event_type")
            .and_then(Value::as_str)
            .unwrap_or("unknown");

        // Print the best ask and best bid if present
        // These are the prices you'd use for arb detection later
        let best_ask = get_best_price(event, "asks");
        let best_bid = get_best_price(event, "bids");

        println!(
            "[WS] token={} type={} best_ask={} best_bid={}",
            &asset_id[..asset_id.len().min(16)], // truncate long token IDs for readability
            event_type,
            best_ask.as_deref().unwrap_or("—"),
            best_bid.as_deref().unwrap_or("—"),
        );
    }
}

// ── Extract the best price from asks or bids array ───────────────────────────
// Polymarket sends price levels as an array — best ask is the lowest ask,
// best bid is the highest bid.
fn get_best_price(event: &Value, side: &str) -> Option<String> {
    let levels = event.get(side)?.as_array()?;
    if levels.is_empty() {
        return None;
    }
    // Each level is { "price": "0.55", "size": "100" }
    levels
        .iter()
        .filter_map(|l| l.get("price").and_then(Value::as_str))
        .next()
        .map(str::to_string)
}
