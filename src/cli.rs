use crate::positions::Position;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "ESPN Fantasy Football Stats CLI",
    about = "A CLI tool for collecting fantasy football stats from ESPN"
)]
pub enum ESPN {
    Get {
        /// League ID (falls back to ESPN_FFL_LEAGUE_ID)
        #[structopt(long, short = "l")]
        league_id: Option<u32>,

        /// Filter by player last name (substring)
        #[structopt(long, short = "n")]
        player_name: Option<String>,

        /// Positions (repeatable): -p QB -p K -p D
        #[structopt(long, short = "p")]
        positions: Option<Vec<Position>>,

        /// Season (default 2025)
        #[structopt(long, short = "y", default_value = "2025")]
        season: u16,

        /// Week (1..18)
        #[structopt(long, short = "w")]
        week: u16,

        /// Print request URL and headers
        #[structopt(long)]
        debug: bool,
    },
}
