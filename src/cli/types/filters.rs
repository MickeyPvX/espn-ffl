//! Filter types for ESPN Fantasy Football CLI commands.

use std::fmt;

/// Filter for player injury status in CLI commands.
///
/// Allows filtering players by their current injury designation.
/// Some filters work server-side with ESPN API for efficiency, while others
/// require client-side filtering.
///
/// # Server-side vs Client-side Filtering
///
/// - **Server-side** (efficient): `Active`, `Injured`
/// - **Client-side** (less efficient): Specific statuses like `Out`, `Doubtful`, etc.
#[derive(Debug, Clone, clap::ValueEnum)]
pub enum InjuryStatusFilter {
    /// Players who are active/healthy
    Active,
    /// Players with any injury status (questionable, doubtful, out, etc.)
    Injured,
    /// Players listed as "Out"
    Out,
    /// Players listed as "Doubtful"
    Doubtful,
    /// Players listed as "Questionable"
    Questionable,
    /// Players listed as "Probable"
    Probable,
    /// Players listed as "Day to Day"
    DayToDay,
    /// Players on Injured Reserve
    IR,
}

impl fmt::Display for InjuryStatusFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            InjuryStatusFilter::Active => "Active",
            InjuryStatusFilter::Injured => "Injured",
            InjuryStatusFilter::Out => "Out",
            InjuryStatusFilter::Doubtful => "Doubtful",
            InjuryStatusFilter::Questionable => "Questionable",
            InjuryStatusFilter::Probable => "Probable",
            InjuryStatusFilter::DayToDay => "Day to Day",
            InjuryStatusFilter::IR => "IR",
        };
        write!(f, "{}", s)
    }
}

/// Filter for player roster status in CLI commands.
///
/// Allows filtering players by whether they are currently rostered
/// on a fantasy team in your league.
#[derive(Debug, Clone, clap::ValueEnum)]
pub enum RosterStatusFilter {
    /// Players currently rostered on any team
    Rostered,
    /// Free agents (unrostered players)
    FA,
}

impl fmt::Display for RosterStatusFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            RosterStatusFilter::Rostered => "Rostered",
            RosterStatusFilter::FA => "FA",
        };
        write!(f, "{}", s)
    }
}
