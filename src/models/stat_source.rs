//! Stat source for ESPN player stats
//!     - Actual (game results):    statSourceId = 0
//!     - Projected:                statSourceId = 1

#[derive(Clone, Copy, Debug)]
pub enum StatSource {
    Actual,
    Projected,
}

impl StatSource {
    /// ESPN statSourceId corresponding to this source
    pub fn id(self) -> u64 {
        match self {
            StatSource::Actual => 0,
            StatSource::Projected => 1,
        }
    }
}
