use std::str::FromStr;

use serde_json::Value;
use structopt::StructOpt;

const FFL_BASE_URL: &str = "https://lm-api-reads.fantasy.espn.com/apis/v3/games/ffl";

#[derive(Debug)]
enum Position {
    D,
    FLEX,
    K,
    RB,
    QB,
    TE,
    WR,
}

impl FromStr for Position {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "D" | "D/ST" | "DEF" => Ok(Self::D),
            "FLEX" => Ok(Self::FLEX),
            "K" => Ok(Self::K),
            "RB" => Ok(Self::RB),
            "QB" => Ok(Self::QB),
            "TE" => Ok(Self::TE),
            "WR" => Ok(Self::WR),
            _ => Err(format!("Unrecognized player position: {:?}", s))
        }
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = "ESPN Fantasy Football Stats CLI", about = "A CLI tool for collecting fantasy football stats from ESPN")]
enum ESPN {
    Get {
        #[structopt(short)]
        _position: Position
    }
}

async fn request(league_id: &str) -> Result<Value, reqwest::Error> {
    let res_json = reqwest::get(format!("{}/seasons/2025/segments/0/leagues/{}", FFL_BASE_URL, league_id))
        .await?
        .json::<serde_json::Value>()
        .await?;

    Ok(res_json)
}

#[tokio::main]
async fn main() {
    let league_id = std::env::var("ESPN_FFL_LEAGUE_ID").unwrap();
    println!("{:?}", request(league_id.as_str()).await.unwrap());
}
