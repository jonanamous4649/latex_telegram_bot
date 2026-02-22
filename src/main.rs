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
                println!("Received 'fetch games' command, running...");
                let tag_ids = vec!["100149", "101178", "100351", "450", "745", "100350", "82", "101674", "102779", "100639", "864", "101232", "102123"];
                let now_str = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
                let now = Utc::now();                                   // current time
                let eight_hours_later = now + Duration::days(2);       // time filter
                println!("{}", now);
                println!("{}\n", eight_hours_later);
                
                let mut filtered = Vec::new();
                for tag_id in tag_ids {
                    let url = format!(
                        "https://gamma-api.polymarket.com/events?limit=50&end_date_min={}&closed=false&tag_id={}",
                        now_str, tag_id
                    );
                    let response = ureq::get(&url).call().unwrap();
                    let mut body = String::new();
                    response.into_reader().read_to_string(&mut body).unwrap();

                    // ============================================================
                    // DATA EXTRACTION AND .JSON OUTPUT
                    // ============================================================
                    let events: Vec<Value> = serde_json::from_str(&body).unwrap();         // all data drom gamma API

                    // extract ID, TITLE, SLUG, END-DATE
                    for event in events {
                        let id = event.get("id").unwrap().as_str().unwrap();
                        
                        // let event_tags: Vec<String> = event.get("tags")
                        //     .and_then(|t| t.as_array())
                        //     .map(|arr| arr.iter()
                        //         .filter_map(|t| {
                        //             let id = t.get("id").and_then(Value::as_str)?;
                        //             let label = t.get("label").and_then(Value::as_str)?;
                        //             Some(format!("{}:{}", id, label))
                        //         })
                        //         .collect())
                        //     .unwrap_or_default();

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
                        
                        let title = event.get("title").unwrap().as_str().unwrap();
                        
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
                        
                        // check if within 8 hours
                        if end_date > now && end_date <= eight_hours_later {
                            let end_date_hst = utc_to_hst(end_date_str);
                            let simplified = serde_json::json!({
                                "id": id,
                                "tag_id": event_tags,
                                "title": title,
                                "slug": slug,
                                "endDateHST": end_date_hst
                            });
                            // println!("{}", end_date_hst);
                            // println!("{}\n", slug);

                            println!("TITLE: {} | TAGS: {:?}\n", title, event_tags);

                            filtered.push(simplified);
                        }
                    }
                }
                
                let result = serde_json::to_string_pretty(&filtered).unwrap();
                fs::create_dir_all("events").unwrap();
                write("events/polymarket_btc_events.json", result).unwrap();
            
            }

                    
        }

        std::thread::sleep(std::time::Duration::from_secs(2));
        
    }

    // let tag_ids = vec!["100149", "101178", "100351", "450", "745", "100350", "82", "101674", "102779", "100639", "864", "101232", "102123"];
    // let now_str = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    // let now = Utc::now();                                   // current time
    // let eight_hours_later = now + Duration::days(2);       // time filter
    // println!("{}", now);
    // println!("{}\n", eight_hours_later);
    
    // let mut filtered = Vec::new();

    // for tag_id in tag_ids {
    //     let url = format!(
    //         "https://gamma-api.polymarket.com/events?limit=50&end_date_min={}&closed=false&tag_id={}",
    //         now_str, tag_id
    //     );
    //     let response = ureq::get(&url).call().unwrap();
    //     let mut body = String::new();
    //     response.into_reader().read_to_string(&mut body).unwrap();

    //     // ============================================================
    //     // DATA EXTRACTION AND .JSON OUTPUT
    //     // ============================================================
    //     let events: Vec<Value> = serde_json::from_str(&body).unwrap();         // all data drom gamma API

    //     // extract ID, TITLE, SLUG, END-DATE
    //     for event in events {
    //         let id = event.get("id").unwrap().as_str().unwrap();
            
    //         // let event_tags: Vec<String> = event.get("tags")
    //         //     .and_then(|t| t.as_array())
    //         //     .map(|arr| arr.iter()
    //         //         .filter_map(|t| {
    //         //             let id = t.get("id").and_then(Value::as_str)?;
    //         //             let label = t.get("label").and_then(Value::as_str)?;
    //         //             Some(format!("{}:{}", id, label))
    //         //         })
    //         //         .collect())
    //         //     .unwrap_or_default();

    //         let event_tags: Vec<String> = event.get("tags")
    //             .and_then(|t| t.as_array())
    //             .map(|arr| arr.iter()
    //                 .filter_map(|t| {
    //                     let id = t.get("id").and_then(Value::as_str)?;
    //                     let label = t.get("label").and_then(Value::as_str)?;
    //                     Some(format!("{}:{}", id, label))
    //                 })
    //                 .collect())
    //             .unwrap_or_default();

    //         let is_game = event_tags.iter().any(|t| t.starts_with("100639:"));

    //         if !is_game {
    //             continue;
    //         }
            
    //         let title = event.get("title").unwrap().as_str().unwrap();
            
    //         let slug = event.get("slug").unwrap().as_str().unwrap();
            
    //         // extract endDate from event info
    //         let end_date_str = match event.get("endDate")
    //             .and_then(Value::as_str) {
    //                 Some(v) => v,
    //                 None => continue,
    //             };
    //         let end_date: DateTime<Utc> = DateTime::parse_from_rfc3339(end_date_str)
    //             .unwrap()
    //             .with_timezone(&Utc);
            
    //         // check if within 8 hours
    //         if end_date > now && end_date <= eight_hours_later {
    //             let end_date_hst = utc_to_hst(end_date_str);
    //             let simplified = serde_json::json!({
    //                 "id": id,
    //                 "tag_id": event_tags,
    //                 "title": title,
    //                 "slug": slug,
    //                 "endDateHST": end_date_hst
    //             });
    //             // println!("{}", end_date_hst);
    //             // println!("{}\n", slug);

    //             println!("TITLE: {} | TAGS: {:?}\n", title, event_tags);

    //             filtered.push(simplified);
    //         }
    //     }
    //     // ============================================================
    // }

    // let result = serde_json::to_string_pretty(&filtered).unwrap();
    // fs::create_dir_all("events").unwrap();
    // write("events/polymarket_btc_events.json", result).unwrap();
    // ================================================================================

    // ================================================================================
    // SEND MESSAGE TO TG
    // ================================================================================
    // let bot_token = "8205762687:AAEMPfLccVrzukLQApkyrxopDBaU4qKw71g";
    // let chat_id = "8363439123";
    // let message = "Hello from Rust!";

    // let url = format!(
    //     "https://api.telegram.org/bot{}/sendMessage?chat_id={}&text={}",
    //     bot_token,
    //     chat_id,
    //     message
    // );

    // Client::new().get(&url).send().unwrap();

}
