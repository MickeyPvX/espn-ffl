use std::{collections::HashMap, error::Error, str::FromStr};

use reqwest::{
    header::{HeaderMap, HeaderValue, ACCEPT, COOKIE}, Client
};
use serde_json::{Value, json};
use structopt::StructOpt;

const FFL_BASE_URL: &str = "https://lm-api-reads.fantasy.espn.com/apis/v3/games/ffl";
const POSITION_MAP: [(Position, usize); 7] = [
    (Position::D, 16),
    (Position::FLEX, 23),
    (Position::K, 17),
    (Position::RB, 2),
    (Position::QB, 0),
    (Position::TE, 6),
    (Position::WR, 4),
];

#[derive(Debug, Eq, Hash, PartialEq)]
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
            _ => Err(format!("Unrecognized player position: {:?}", s)),
        }
    }
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "ESPN Fantasy Football Stats CLI",
    about = "A CLI tool for collecting fantasy football stats from ESPN"
)]
enum ESPN {
    Get {
        #[structopt(long, short)]
        league_id: Option<u32>,
        #[structopt(long, short)]
        position: Position,
        #[structopt(default_value = "2025", long, short)]
        season: u16,
    },
}

async fn request(
    params: &[(&str, &str)],
    filters: HeaderMap,
    season: u16,
) -> Result<Value, reqwest::Error>
{
    let client = Client::new();

    let builder = client
        .get(format!("{}/seasons/{}/players", FFL_BASE_URL, season))
        .headers(filters)
        .query(&params);

    let debug_builder = builder.try_clone().unwrap().build()?;
    println!(
        "Sending: {}\nHeaders: {:#?}",
        debug_builder.url(),
        debug_builder.headers()
    );

    let res_json = builder.send().await?.error_for_status()?.json().await?;

    Ok(res_json)
}

fn get_slot_map() -> HashMap<Position, usize> {
    POSITION_MAP.into_iter().collect()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let app = ESPN::from_args();
    let (league_id, position, season) = match app {
        ESPN::Get {
            league_id,
            position,
            season,
        } => {
            let league_id_parsed = match league_id {
                Some(id) => id,
                None => std::env::var("ESPN_FFL_LEAGUE_ID")?.parse::<u32>()?,
            };

            (league_id_parsed, position, season)
        }
        _ => panic!("Not a valid command!"),
    };
    let league_id_param = league_id.to_string();
    let slot_id = get_slot_map()[&position];
    let default_params = [
        ("forLeagueId", league_id_param.as_str()),
        ("scoringPeriodId", "1"),
        ("view", "kona_player_info"),
    ];
    let mut filters = HeaderMap::new();
    let default_filters = json!(
        {
            "filterActive":{"value":true},
            "filterSlotIds": {"value": [slot_id]}
        }
    );
    let cookie_data = format!("SWID={}; espn_s2={}", std::env::var("ESPN_SWID")?, std::env::var("ESPN_S2")?);
    filters.insert(ACCEPT, HeaderValue::from_static("application/json"));
    filters.insert(COOKIE, HeaderValue::from_str(&cookie_data)?);
    filters.insert(
        "x-fantasy-filter",
        HeaderValue::from_str(&default_filters.to_string())?,
    );

    let value = request(&default_params, filters, season).await?;
    let arr = value.as_array().unwrap();

    for player in arr {
        println!("{}", player.get("fullName").unwrap());
    }

    Ok(())
}
