//! Update all player data command for bulk data population
//!
//! This command efficiently updates all player data (both actual and projected)
//! for multiple weeks by reusing the existing player-data command logic.

use crate::{LeagueId, Result, Season, Week};

use super::{
    common::CommandParamsBuilder,
    player_data::{handle_player_data, PlayerDataParams},
    resolve_league_id,
};

/// Update all player data (actual and projected) for weeks 1 through the specified week
///
/// This command efficiently populates the database with complete historical data
/// by calling the existing player-data command for both actual and projected data.
///
/// # Arguments
/// * `season` - The season year
/// * `through_week` - Update data through this week (inclusive)
/// * `league_id` - Optional league ID override
/// * `verbose` - Show detailed progress information
pub async fn handle_update_all_data(
    season: Season,
    through_week: Week,
    league_id: Option<LeagueId>,
    verbose: bool,
) -> Result<()> {
    let league_id = resolve_league_id(league_id)?;

    if verbose {
        println!(
            "Updating all player data for Season {} through Week {}",
            season.as_u16(),
            through_week.as_u16()
        );
        println!("League ID: {}", league_id.as_u32());
    }

    let mut total_weeks_processed = 0;

    // Process each week from 1 to through_week
    for week_num in 1..=through_week.as_u16() {
        let week = Week::new(week_num);

        if verbose {
            println!("\n--- Processing Week {} ---", week_num);
        } else {
            println!("Processing Week {}...", week_num);
        }

        // Fetch actual data first
        if verbose {
            println!("Fetching actual player data...");
        }
        let actual_params = PlayerDataParams::new(season, week, false)
            .with_league_id(league_id)
            .with_refresh();
        handle_player_data(actual_params).await?;

        // Fetch projected data
        if verbose {
            println!("Fetching projected player data...");
        }
        let projected_params = PlayerDataParams::new(season, week, true)
            .with_league_id(league_id)
            .with_refresh();
        handle_player_data(projected_params).await?;

        total_weeks_processed += 1;

        if verbose {
            println!("✓ Week {} complete (actual + projected data)", week_num);
        }
    }

    println!("\n✓ Data update complete!");
    println!("Total weeks processed: {}", total_weeks_processed);

    if verbose {
        println!(
            "\nDatabase now contains complete actual and projected data for weeks 1-{}",
            through_week.as_u16()
        );
        println!("This data can be used for projection analysis and bias correction.");
    }

    Ok(())
}
