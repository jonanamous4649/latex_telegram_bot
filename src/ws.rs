// ws.rs — Polymarket CLOB WebSocket price monitor
//
// Connects to Polymarket's real-time order book stream and prints live
// price updates to the terminal. Runs as a background tokio task.
//
// Polymarket WebSocket docs:
// wss://ws-subscriptions-clob.polymarket.com/ws/market

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio_tungstenite::{connect_async, tungstenite::Message};

const WS_URL: &str = "wss://ws-subscriptions-clob.polymarket.com/ws/market";

// ── Entry point ───────────────────────────────────────────────────────────────
// Takes a list of (token_id, outcome_name) pairs so we can display
// readable names like "Fuego" and "AB3" instead of raw token IDs.
pub async fn run(tokens: Vec<(String, String)>) {
    loop {
        println!("[WS] Connecting to Polymarket...");

        match connect_and_stream(&tokens).await {
            Ok(_) => println!("[WS] Stream ended, reconnecting..."),
            Err(e) => println!("[WS] Connection error: {e}, reconnecting in 5s..."),
        }

        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

// ── Connect, subscribe, and stream messages ───────────────────────────────────
async fn connect_and_stream(tokens: &[(String, String)]) -> Result<(), Box<dyn std::error::Error>> {
    let (mut ws, _) = connect_async(WS_URL).await?;

    // Build lookup map: token_id → outcome_name for display
    // Build token_id list for the subscription message
    let names: HashMap<String, String> = tokens
        .iter()
        .map(|(id, name)| (id.clone(), name.clone()))
        .collect();
    let token_ids: Vec<&String> = tokens.iter().map(|(id, _)| id).collect();

    println!("[WS] Connected — subscribing to {} tokens", token_ids.len());

    let sub_msg = json!({
        "assets_ids": token_ids,
        "type": "market"
    });
    ws.send(Message::Text(sub_msg.to_string())).await?;
    println!("[WS] Subscribed. Streaming live prices...\n");

    // ── State map ─────────────────────────────────────────────────────────────
    // Tracks the latest known market_ask per token so we can always calculate
    // the current sum across both tokens, even when only one side updates.
    let mut ask_state: HashMap<String, f64> = HashMap::new();

    while let Some(msg) = ws.next().await {
        match msg? {
            Message::Text(text) => handle_message(&text, &names, &mut ask_state),
            Message::Ping(data) => { ws.send(Message::Pong(data)).await?; }
            Message::Close(_)   => { println!("[WS] Server closed connection"); break; }
            _ => {}
        }
    }

    Ok(())
}

// ── Route each message by event_type ─────────────────────────────────────────
fn handle_message(text: &str, names: &HashMap<String, String>, ask_state: &mut HashMap<String, f64>) {
    let msg: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => { println!("[WS] Unparseable: {text}"); return; }
    };

    match msg.get("event_type").and_then(Value::as_str) {
        Some("price_change") => print_price_change(&msg, names, ask_state),
        Some("book")         => seed_state_from_book(&msg, ask_state), // seeds initial ask state, no display
        Some("last_trade_price") => {} // on-chain receipt only, ask state unchanged
        Some(other)          => println!("[WS] Unhandled event_type: {other}"),
        None                 => {} // subscription ack, safe to ignore
    }
}

// ── price_change ──────────────────────────────────────────────────────────────
// Fires when the CLOB matches or cancels an order (off-chain).
// Each message contains two entries — one per token — since every event on a
// binary market affects both sides simultaneously.
//
// We update ask_state on every message so the sum always reflects the
// latest known ask for both tokens even when only one side changes.
fn print_price_change(msg: &Value, names: &HashMap<String, String>, ask_state: &mut HashMap<String, f64>) {
    let changes = match msg.get("price_changes").and_then(Value::as_array) {
        Some(c) => c,
        None    => return,
    };

    // Update state for every token in this message
    for change in changes {
        let id  = change.get("asset_id").and_then(Value::as_str).unwrap_or("");
        let ask = change.get("best_ask").and_then(Value::as_str)
            .and_then(|v| v.parse::<f64>().ok());
        if let (id, Some(ask)) = (id, ask) {
            ask_state.insert(id.to_string(), ask);
        }
    }

    // Need exactly two entries to display a paired line
    if changes.len() < 2 { return; }

    let a = &changes[0];
    let b = &changes[1];

    let id_a   = a.get("asset_id").and_then(Value::as_str).unwrap_or("");
    let id_b   = b.get("asset_id").and_then(Value::as_str).unwrap_or("");
    let name_a = names.get(id_a).map(|s| s.as_str()).unwrap_or("Token A");
    let name_b = names.get(id_b).map(|s| s.as_str()).unwrap_or("Token B");
    let side_a = a.get("side").and_then(Value::as_str).unwrap_or("?");
    let side_b = b.get("side").and_then(Value::as_str).unwrap_or("?");
    let size_a = a.get("size").and_then(Value::as_str).unwrap_or("?");

    // Use state map for the sum — guaranteed to use latest known ask for both tokens
    let ask_a = ask_state.get(id_a).copied();
    let ask_b = ask_state.get(id_b).copied();
    let sum   = ask_a.zip(ask_b).map(|(a, b)| a + b);

    let size_label = if size_a == "0" { "CANCEL".to_string() } else { size_a.to_string() };
    let ask_a_str  = ask_a.map(|v| format!("{v:.2}")).unwrap_or("—".to_string());
    let ask_b_str  = ask_b.map(|v| format!("{v:.2}")).unwrap_or("—".to_string());
    let sum_str    = sum.map(|s| format!("{s:.2}")).unwrap_or("—".to_string());
    let arb_flag   = sum.map(|s| if s < 0.98 { " ← ARB" } else { "" }).unwrap_or("");

    println!(
        "  {side_a:<4} {name_a:<20} ask={ask_a_str:<5}  |  {side_b:<4} {name_b:<20} ask={ask_b_str:<5}  |\nsum={sum_str}  size={size_label}{arb_flag}",
    );
}

// ── seed_state_from_book ──────────────────────────────────────────────────────
// Silently populates ask_state from the initial book snapshot Polymarket sends
// on subscribe. This ensures the very first price_change line shows real ask
// values instead of blanks.
//
// Book asks are sorted highest→lowest so best ask (lowest) is the LAST entry.
fn seed_state_from_book(msg: &Value, ask_state: &mut HashMap<String, f64>) {
    let id = match msg.get("asset_id").and_then(Value::as_str) {
        Some(id) => id,
        None     => return,
    };

    let best_ask = msg
        .get("asks")
        .and_then(Value::as_array)
        .and_then(|arr| arr.last())       // last entry = lowest ask = best ask
        .and_then(|l| l.get("price"))
        .and_then(Value::as_str)
        .and_then(|v| v.parse::<f64>().ok());

    if let Some(ask) = best_ask {
        ask_state.insert(id.to_string(), ask);
    }
}
