//! Analysis operations for projection accuracy and performance estimation

use super::{models::*, schema::PlayerDatabase};
use crate::{PlayerId, Season, Week};
use anyhow::Result;
use rusqlite::params;

impl PlayerDatabase {
    /// Get players with the biggest projection errors (over/under estimated)
    pub fn get_projection_analysis(
        &self,
        season: Season,
        week: Option<Week>,
        limit: Option<u32>,
    ) -> Result<Vec<ProjectionAnalysis>> {
        let mut query = String::from(
            "SELECT p.name, p.position, p.team,
                    AVG(s.projected_points - s.actual_points) as avg_error,
                    COUNT(*) as games_count
             FROM players p
             JOIN player_weekly_stats s ON p.player_id = s.player_id
             WHERE s.season = ?
               AND s.projected_points IS NOT NULL
               AND s.actual_points IS NOT NULL",
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(season.as_u16())];

        if let Some(w) = week {
            query.push_str(" AND s.week < ?");
            params.push(Box::new(w.as_u16()));
        }

        query.push_str(" GROUP BY p.player_id, p.name, p.position, p.team ORDER BY avg_error DESC");

        if let Some(l) = limit {
            query.push_str(" LIMIT ?");
            params.push(Box::new(l));
        }

        let mut stmt = self.conn.prepare(&query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(&param_refs[..], |row| {
            Ok(ProjectionAnalysis {
                name: row.get(0)?,
                position: row.get(1)?,
                team: row.get(2)?,
                avg_error: row.get(3)?,
                games_count: row.get(4)?,
            })
        })?;

        let mut analysis = Vec::new();
        for row in rows {
            analysis.push(row?);
        }
        Ok(analysis)
    }

    /// Estimate performance for a specific week based on ESPN projections and historical bias
    pub fn estimate_week_performance(
        &self,
        season: Season,
        target_week: Week,
        projected_points_data: &[(PlayerId, f64)], // ESPN projections for target week
        limit: Option<u32>,
        bias_strength: f64, // 0.0 = no adjustment, 1.0 = full bias correction, >1.0 = amplified
    ) -> Result<Vec<PerformanceEstimate>> {
        let mut estimates = Vec::new();

        for (player_id, espn_projection) in projected_points_data
            .iter()
            .take(limit.map(|l| l as usize).unwrap_or(usize::MAX))
        {
            // Get player info first
            let mut player_stmt = self
                .conn
                .prepare("SELECT name, position, team FROM players WHERE player_id = ?")?;

            let player_info = player_stmt.query_row(params![player_id.as_i64()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            });

            let (name, position, team) = match player_info {
                Ok(info) => info,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    // Player not found in database, skip
                    continue;
                }
                Err(e) => return Err(e.into()),
            };

            // Get individual bias values for this player
            // Include all weeks with both projected and actual data
            let mut bias_stmt = self.conn.prepare(
                "SELECT s.projected_points, s.actual_points, (s.projected_points - s.actual_points) as bias
                 FROM player_weekly_stats s
                 WHERE s.player_id = ?
                   AND s.season = ?
                   AND s.week < ?
                   AND s.projected_points IS NOT NULL
                   AND s.actual_points IS NOT NULL",
            )?;

            let bias_rows = bias_stmt.query_map(
                params![player_id.as_i64(), season.as_u16(), target_week.as_u16()],
                |row| {
                    Ok((
                        row.get::<_, f64>(0)?, // projected_points
                        row.get::<_, f64>(1)?, // actual_points
                        row.get::<_, f64>(2)?, // bias
                    ))
                },
            )?;

            let mut bias_values = Vec::new();
            for bias_result in bias_rows {
                let (projected, actual, bias) = bias_result?;
                // Skip weeks where both projected and actual are zero (BYE weeks, didn't play)
                if projected == 0.0 && actual == 0.0 {
                    continue;
                }
                bias_values.push(bias);
            }

            let games_count = bias_values.len() as u32;
            if games_count == 0 {
                // No historical data, skip to fallback
                continue;
            }

            // Simple approach: Calculate player's average bias (no recency weighting)
            let average_bias = bias_values.iter().sum::<f64>() / bias_values.len() as f64;

            // Start with ESPN's projection
            let base_projection = *espn_projection;

            // If ESPN projects 0 points, don't adjust - player is likely not playing or on bye
            let (bias_adjustment, estimated_points) = if base_projection == 0.0 {
                (0.0, 0.0)
            } else {
                // Simple bias adjustment - trust player-specific patterns
                let sample_factor = games_count as f64 / (games_count as f64 + 2.0);

                // Only limit extreme biases
                let bias_magnitude = average_bias.abs();
                let magnitude_factor = if bias_magnitude > 10.0 {
                    10.0 / bias_magnitude
                } else {
                    1.0
                };

                let adjustment_strength = sample_factor * magnitude_factor;
                let bias_adjustment = -average_bias * adjustment_strength * bias_strength;
                let estimated_points = (base_projection + bias_adjustment).max(0.0);
                (bias_adjustment, estimated_points)
            };

            // Confidence based on pattern consistency
            let bias_variance = if bias_values.len() > 1 {
                bias_values
                    .iter()
                    .map(|&x| (x - average_bias).powi(2))
                    .sum::<f64>()
                    / (bias_values.len() - 1) as f64
            } else {
                0.0
            };

            let bias_std = bias_variance.sqrt();
            let consistency_factor = 1.0 / (1.0 + bias_std / 3.0); // Higher std = lower confidence
            let confidence = (0.3 + 0.5 * consistency_factor).clamp(0.25, 0.85);

            // Generate simple reasoning
            let reasoning = if base_projection == 0.0 {
                "ESPN projects 0 pts - player not expected to play or on bye week".to_string()
            } else if bias_adjustment.abs() > 1.0 {
                if average_bias > 0.0 {
                    format!(
                        "Avg bias: ESPN overestimates by {:.1} pts ({} games, {:.1} std) - adjusted down {:.1} pts ({}% confidence)",
                        average_bias,
                        games_count,
                        bias_std,
                        bias_adjustment.abs(),
                        (confidence * 100.0) as u8
                    )
                } else {
                    format!(
                        "Avg bias: ESPN underestimates by {:.1} pts ({} games, {:.1} std) - adjusted up {:.1} pts ({}% confidence)",
                        average_bias.abs(),
                        games_count,
                        bias_std,
                        bias_adjustment,
                        (confidence * 100.0) as u8
                    )
                }
            } else {
                format!(
                    "ESPN projection {:.1} pts - minimal bias detected ({} games, {}% confidence)",
                    base_projection,
                    games_count,
                    (confidence * 100.0) as u8
                )
            };

            estimates.push(PerformanceEstimate {
                player_id: *player_id,
                name,
                position,
                team,
                espn_projection: base_projection,
                bias_adjustment,
                estimated_points,
                confidence,
                reasoning,
            });
        }

        // Add fallback for players not found in database but in ESPN data
        for (player_id, espn_projection) in projected_points_data
            .iter()
            .take(limit.map(|l| l as usize).unwrap_or(usize::MAX))
        {
            // Check if we already processed this player
            if estimates.iter().any(|e| e.player_id == *player_id) {
                continue;
            }

            // No historical data, but try to get player info from players table
            let mut player_stmt = self
                .conn
                .prepare("SELECT name, position, team FROM players WHERE player_id = ?")?;

            let (name, position, team) = player_stmt
                .query_row(params![player_id.as_i64()], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                })
                .unwrap_or_else(|_| ("Unknown".to_string(), "Unknown".to_string(), None));

            estimates.push(PerformanceEstimate {
                player_id: *player_id,
                name,
                position,
                team,
                espn_projection: *espn_projection,
                bias_adjustment: 0.0,
                estimated_points: *espn_projection,
                confidence: 0.3,
                reasoning: "No historical data - using ESPN projection".to_string(),
            });
        }

        // Sort by estimated points descending
        estimates.sort_by(|a, b| {
            b.estimated_points
                .partial_cmp(&a.estimated_points)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(estimates)
    }
}
