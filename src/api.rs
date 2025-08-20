use crate::util::Result;
use once_cell::sync::Lazy;
use reqwest::{Client, header::HeaderMap};
use serde_json::Value;

static HTTP: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .user_agent("espn-cli/0.1")
        .build()
        .expect("Client build")
});

const FFL_BASE_URL: &str = "https://lm-api-reads.fantasy.espn.com/apis/v3/games/ffl";

pub async fn fetch_players(
    season: u16,
    params: &[(String, String)],
    headers: &HeaderMap,
    debug: bool,
) -> Result<Value> {
    let url = format!("{}/seasons/{}/players", FFL_BASE_URL, season);

    let builder = HTTP.get(&url).headers(headers.clone()).query(params);

    if debug {
        let req = builder.try_clone().unwrap().build()?;
        eprintln!("URL => {}", req.url());
        eprintln!("HEADERS:");
        for (k, v) in req.headers().iter() {
            eprintln!("  {}: {:?}", k, v);
        }
    }

    let v = builder
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    Ok(v)
}
