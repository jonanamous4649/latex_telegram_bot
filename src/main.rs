use reqwest::blocking::Client;
use ureq;
use std::fs::{self, write};
use serde_json::Value;
use chrono::{DateTime, Utc, Duration};
use chrono_tz::Pacific::Honolulu;
use std::io::Read;

fn utc_to_hst(utc_str: &str) -> String {
    let utc: DateTime<Utc> = utc_str.parse().unwrap();
    let hst = utc.with_timezone(&Honolulu);
    hst.format("%B %d, %Y %I:%M %p HST").to_string()
}

fn main() {
    // ================================================================================
    // RECIEVE COMMAND FROM TG
    // ================================================================================
    let bot_token = "8205762687:AAEMPfLccVrzukLQApkyrxopDBaU4qKw71g";
    let chat_id = "8363439123";
    let mut offset: i64 = 0;

    loop {
        let url = format!(
            "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=30",
            bot_token, offset
        );

        let response = ureq::get(&url).call().unwrap();
        let mut body = String::new();
        response.into_reader().read_to_string(&mut body).unwrap();

        let json: Value = serde_json::from_str(&body).unwrap();
        let updates = json.get("result").and_then(Value::as_array).unwrap();

        for update in updates {
            let update_id = update.get("update_id").and_then(Value::as_i64).unwrap();
            offset = update_id + 1;

            let message_text = update
                .get("message")
                .and_then(|m| m.get("text"))
                .and_then(Value::as_str)
                .unwrap_or("");

            if message_text == "fetch games" {
                println!("Received 'fetch games' command, running...\n");

                let mut message = "Received 'fetch games' command, running...";
                let url = format!(
                    "https://api.telegram.org/bot{}/sendMessage?chat_id={}&text={}",
                    bot_token,
                    chat_id,
                    message
                );

                Client::new().get(&url).send().unwrap();

                // ================================================================================
                // SEARCH PARAMS
                // ================================================================================
                let tag_ids = vec![
                    "100149", "101178", "100351", "450", "745", "100350",
                    "82", "101674", "102779", "100639", "864", "101232", "102123",
                    "64", "65", "100780", "101672", "102366", "102750", "102753",
                    "102754", "102755", "102756", "102758", "102759"
                ];
                let now_str = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
                let now = Utc::now();                                   // current time
                let hours_later = now + Duration::hours(8);       // time filter
                // println!("{}", now);
                // println!("{}\n", hours_later);
                
                let mut filtered = Vec::new();
                for tag_id in tag_ids {
                    let url = format!(
                        "https://gamma-api.polymarket.com/events?limit=50&end_date_min={}&closed=false&tag_id={}",
                        now_str, tag_id
                    );
                    // let url = format!(
                    //     "https://gamma-api.polymarket.com/events?limit=50&closed=false&tag_id=100780",
                    // );
                    let response = ureq::get(&url).call().unwrap();
                    let mut body = String::new();
                    response.into_reader().read_to_string(&mut body).unwrap();

                    // ============================================================
                    // EVENT FINDER
                    // ============================================================
                    let events: Vec<Value> = serde_json::from_str(&body).unwrap();         // all data drom gamma API

                    for event in events {
                        
                        // extracting unique id for game
                        let id = event.get("id").unwrap().as_str().unwrap();

                        // title of the game
                        let title = event.get("title").unwrap().as_str().unwrap();

                        // slug of the game
                        let slug = event.get("slug").unwrap().as_str().unwrap();
                        
                        // extract endDate from event info
                        let end_date_str = match event.get("endDate")
                            .and_then(Value::as_str) {
                                Some(v) => v,
                                None => continue,
                            };
                        let end_date: DateTime<Utc> = DateTime::parse_from_rfc3339(end_date_str)
                            .unwrap()
                            .with_timezone(&Utc);
                        if !(end_date > now && end_date <= hours_later) {
                            continue;
                        }
                        let end_date_hst = utc_to_hst(end_date_str);
                        
                        // filtering for binary 'vs' markets
                        let event_tags: Vec<String> = event.get("tags")
                            .and_then(|t| t.as_array())
                            .map(|arr| arr.iter()
                                .filter_map(|t| {
                                    let id = t.get("id").and_then(Value::as_str)?;
                                    let label = t.get("label").and_then(Value::as_str)?;
                                    Some(format!("{}:{}", id, label))
                                })
                                .collect())
                            .unwrap_or_default();

                        let is_game = event_tags.iter().any(|t| t.starts_with("100639:"));

                        if !is_game {
                            continue;
                        }
                        
                        // getting clob_token_ids
                        let markets = event.get("markets").and_then(Value::as_array);
                        let mut market_entries: Vec<Value> = Vec::new();
                        // let mut binary_tokens: Vec<Value> = Vec::new();
                        // let mut outcomes: Vec<Value> = Vec::new();
                        if let Some(markets) = markets {
                            println!("EVENT: {} | Total markets: {}", title, markets.len());
                            println!("EndDate: {}", end_date_hst);

                            for market in markets {
                                let sports_market_type = market.get("sportsMarketType").and_then(Value::as_str).unwrap_or("N/A");
                                if sports_market_type != "moneyline" {
                                    continue;
                                }

                                let question = market.get("question").and_then(Value::as_str).unwrap_or("N/A");
                                let clob_token_ids = market.get("clobTokenIds").and_then(Value::as_str).unwrap_or("N/A");
                                let binary_tokens: Vec<Value> = serde_json::from_str(clob_token_ids).unwrap_or_default();
                                let outcomes: Vec<Value> = serde_json::from_str(
                                    market.get("outcomes").and_then(Value::as_str).unwrap_or("[]")).unwrap_or_default();
                                
                                println!("  Question: {}", question);
                                println!("  sportsMarketType: {}", sports_market_type);
                                // println!("  clobTokenIds: {}", clob_token_ids);

                                // ================================================================================
                                // ORDERBOOK PRICES
                                // ================================================================================
                                let mut side_entries: Vec<Value> = Vec::new();
                                for (i, token) in binary_tokens.iter().enumerate() {
                                    let token_str = match token.as_str() {
                                        Some(v) => v,
                                        None => continue,
                                    };

                                    let clob_url = format!("https://clob.polymarket.com/book?token_id={}", token_str);
                                    let clob_response = match ureq::get(&clob_url).call() {
                                        Ok(r) => r,
                                        Err(_) => continue,
                                    };
                                    let mut clob_body = String::new();
                                    clob_response.into_reader().read_to_string(&mut clob_body).unwrap();

                                    let book: Value = serde_json::from_str(&clob_body).unwrap();
                                    let best_ask = book.get("asks")
                                        .and_then(Value::as_array)
                                        .and_then(|b| b.last())
                                        .and_then(|b| b.get("price"))
                                        .and_then(Value::as_str)
                                        .unwrap_or("N/A");

                                    if best_ask == "N/A" {
                                        side_entries.clear();
                                        break;
                                    }

                                    let outcome = outcomes.get(i).and_then(Value::as_str).unwrap_or("Unkown");
                                    println!(" {} | Ask: {}", outcome, best_ask);

                                    side_entries.push(serde_json::json!({
                                        "outcome": outcome,
                                        // "token_id": token_str,
                                        "best_ask": best_ask,
                                    }))
                                }
                                market_entries.push(serde_json::json!({
                                    "question": question,
                                    "sides": side_entries,
                                    "sports_market_type": sports_market_type,
                                }))
                            }   
                        }
                        
                        // ================================================================================
                        // JSON FILE FORMAT
                        // ================================================================================
                        let simplified = serde_json::json!({
                            "id": id,
                            "tag_id": event_tags,
                            "title": title,
                            "slug": slug,
                            "endDateHST": end_date_hst,
                            "market_entires": market_entries 
                        });

                        println!("TAGS: {:?}\n", event_tags);

                        filtered.push(simplified);
                        // }
                    }
                }
                
                // ================================================================================
                // SAVING JSON FILE
                // ================================================================================
                let result = serde_json::to_string_pretty(&filtered).unwrap();
                fs::create_dir_all("events").unwrap();
                write("events/polymarket_btc_events.json", result).unwrap();
                
                message = ".json file updated!";
                let url = format!(
                    "https://api.telegram.org/bot{}/sendMessage?chat_id={}&text={}",
                    bot_token,
                    chat_id,
                    message.replace(" ", "%20")
                );

                ureq::get(&url).call().unwrap();

            }

                    
        }

        std::thread::sleep(std::time::Duration::from_secs(2));
        
    }

}
