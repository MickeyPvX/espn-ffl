//! Storage layer for the ESPN Fantasy Football CLI
//!
//! This module provides a clean abstraction over the SQLite database,
//! organized into logical components:
//! - `models`: Data structures
//! - `schema`: Database connection and schema management
//! - `queries`: Basic CRUD operations
//! - `analysis`: Complex analysis and projection operations

pub mod analysis;
pub mod models;
pub mod queries;
pub mod schema;

#[cfg(test)]
mod tests;

// Re-export the main types and database struct for easy access
pub use models::*;
pub use schema::PlayerDatabase;
