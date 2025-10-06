//! Core utilities for the ESPN Fantasy Football CLI
//!
//! This module consolidates common utilities that are used across
//! the application:
//! - `cache`: File system caching utilities
//! - `filters`: ESPN API filter structures and utilities

pub mod cache;
pub mod filters;

// Re-export commonly used items for convenience
pub use cache::{league_settings_path, try_read_to_string, write_string};
pub use filters::{build_players_filter, IntoHeaderValue, PlayersFilter, Val};
