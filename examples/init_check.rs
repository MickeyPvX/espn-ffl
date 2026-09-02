//! Manual check: run snapshot recovery over a captured INIT blob.
//!
//! Usage: cargo run --example init_check -- <blob.bin> <league_id> <team_id>
use espn_ffl::espn::live_draft::recover_prior_picks;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&a[1])?;
    let league: u32 = a[2].parse()?;
    let team: u32 = a[3].parse()?;

    let picks = recover_prior_picks(&bytes, league);
    println!("recovered {} picks", picks.len());
    let negative: Vec<_> = picks.iter().filter(|p| p.player_id.as_i64() < 0).collect();
    println!("negative player ids (team defences): {}", negative.len());
    for p in &negative {
        println!(
            "   team {:>3}  slot {:>3}  player {}",
            p.team_id, p.slot_id, p.player_id
        );
    }
    println!("\nmy team ({}) roster:", team);
    for p in picks.iter().filter(|p| p.team_id == team) {
        println!("   slot {:>3}  player {}", p.slot_id, p.player_id);
    }
    Ok(())
}
