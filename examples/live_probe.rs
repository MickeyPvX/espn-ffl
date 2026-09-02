//! Manual probe: join a live draft room and print events.
//!
//! Usage: cargo run --example live_probe -- <league_id> <team_id> [seconds]
//!
//! Joining takes over the team's draft session: ESPN allows one connection per team.
use espn_ffl::espn::live_draft::{DraftEvent, LiveDraftSession};
use espn_ffl::{LeagueId, Season};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let league = LeagueId::new(args[1].parse()?);
    let team: u32 = args[2].parse()?;
    let secs: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(30);

    let session = LiveDraftSession::open(league, Season::new(2026), team).await?;
    println!("authenticated; joining room for team {}", session.team_id());
    let mut stream = session.subscribe().await?;
    println!("connected — reading for {}s\n", secs);

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(secs);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, stream.next_event()).await {
            Err(_) => break,
            Ok(Ok(None)) => {
                println!("<< stream closed by server");
                break;
            }
            Ok(Err(e)) => {
                println!("<< stream error: {e}");
                break;
            }
            Ok(Ok(Some(event))) => match event {
                DraftEvent::Other { verb, .. } => println!("   ({verb})"),
                other => println!(">> {:?}", other),
            },
        }
    }
    Ok(())
}
