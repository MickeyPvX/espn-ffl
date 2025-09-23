//! Analysis operations for projection accuracy and performance estimation

use super::{models::*, schema::PlayerDatabase};
use crate::cli::types::{PlayerId, Season, Week};
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

            let player_info = player_stmt.query_row(params![player_id.as_u64()], |row| {
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
            let mut bias_stmt = self.conn.prepare(
                "SELECT (s.projected_points - s.actual_points) as bias
                 FROM player_weekly_stats s
                 WHERE s.player_id = ?
                   AND s.season = ?
                   AND s.week < ?
                   AND s.projected_points IS NOT NULL
                   AND s.actual_points IS NOT NULL",
            )?;

            let bias_rows = bias_stmt.query_map(
                params![player_id.as_u64(), season.as_u16(), target_week.as_u16()],
                |row| row.get::<_, f64>(0),
            )?;

            let mut bias_values = Vec::new();
            for bias_result in bias_rows {
                bias_values.push(bias_result?);
            }

            let games_count = bias_values.len() as u32;
            if games_count == 0 {
                // No historical data, skip to fallback
                continue;
            }

            // Calculate average bias
            let avg_bias: f64 = bias_values.iter().sum::<f64>() / bias_values.len() as f64;

            // Calculate standard deviation manually
            let variance: f64 = if bias_values.len() > 1 {
                let sum_squared_diffs: f64 = bias_values
                    .iter()
                    .map(|&bias| (bias - avg_bias).powi(2))
                    .sum();
                sum_squared_diffs / (bias_values.len() - 1) as f64 // Sample standard deviation
            } else {
                0.0
            };
            let stddev_bias = variance.sqrt();

            // Start with ESPN's projection
            let base_projection = *espn_projection;

            // Adjust based on historical bias
            let bias_adjustment = if games_count >= 2 && avg_bias.abs() > 0.25 {
                // Calculate base correction factor based on sample size (less conservative)
                let sample_factor = if games_count >= 5 {
                    1.0 // Full confidence with 5+ games
                } else if games_count >= 3 {
                    0.8 // High confidence with 3-4 games
                } else {
                    0.5 // Moderate confidence with 2 games
                };

                // Apply user-specified bias strength multiplier
                -avg_bias * sample_factor * bias_strength
            } else {
                0.0
            };

            let estimated_points = (base_projection + bias_adjustment).max(0.0);

            // Calculate confidence based on sample size and consistency
            let sample_confidence: f64 = if games_count >= 5 {
                0.8
            } else if games_count >= 3 {
                0.6
            } else {
                0.3 // Low confidence with limited data
            };

            // Adjust confidence based on consistency (lower std dev = higher confidence)
            let consistency_factor = if stddev_bias > 0.0 {
                // High standard deviation (inconsistent) reduces confidence
                // We use a sigmoid-like function to map stddev to a multiplier
                let normalized_stddev = (stddev_bias / 10.0).min(2.0); // Cap at reasonable range
                1.0 / (1.0 + normalized_stddev.powi(2)) // Returns 0.2 to 1.0 range
            } else {
                1.0 // Perfect consistency
            };

            let confidence = (sample_confidence * consistency_factor).max(0.1).min(1.0);

            // Generate reasoning
            let reasoning = if games_count < 3 {
                format!(
                    "Limited data ({} games) - using ESPN projection",
                    games_count
                )
            } else if bias_adjustment.abs() > 1.0 {
                if avg_bias > 0.0 {
                    format!(
                        "ESPN typically overestimates by {:.1} pts, adjusted down {:.1} pts",
                        avg_bias,
                        bias_adjustment.abs()
                    )
                } else {
                    format!(
                        "ESPN typically underestimates by {:.1} pts, adjusted up {:.1} pts",
                        avg_bias.abs(),
                        bias_adjustment
                    )
                }
            } else {
                format!(
                    "ESPN projection {:.1} pts - minimal bias detected",
                    base_projection
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

            // No historical data, use ESPN projection as-is
            estimates.push(PerformanceEstimate {
                player_id: *player_id,
                name: "Unknown".to_string(),
                position: "Unknown".to_string(),
                team: None,
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