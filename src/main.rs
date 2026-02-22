use reqwest::blocking::Client;

fn main() {
    let bot_token = "8205762687:AAEMPfLccVrzukLQApkyrxopDBaU4qKw71g";
    let chat_id = "8363439123";
    let message = "Hello from Rust!";

    let url = format!(
        "https://api.telegram.org/bot{}/sendMessage?chat_id={}&text={}",
        bot_token,
        chat_id,
        message
    );

    Client::new().get(&url).send().unwrap();
}
