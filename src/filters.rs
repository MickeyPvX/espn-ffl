//! Serializable filter types for ESPN's `x-fantasy-filter` header.

use reqwest::header::HeaderValue;
use serde::Serialize;
use std::error::Error;

/// Generic wrapper `{ "value": T }` used by ESPN filters.
#[derive(Serialize)]
pub struct Val<T> {
    /// The wrapped filter payload.
    pub value: T,
}

/// Flexible filter with optional fields (root-level).
///
/// ESPN ignores most filters unless `filterActive` is present and `true`.
#[derive(Serialize, Default)]
pub struct Filter {
    /// Enable filtering; required for other filters to take effect.
    #[serde(rename = "filterActive",   skip_serializing_if = "Option::is_none")]
    filter_active:   Option<Val<bool>>,
    /// Filter by player last name (substring).
    #[serde(rename = "filterName",     skip_serializing_if = "Option::is_none")]
    filter_name:     Option<Val<String>>,
    /// Filter by ESPN slot IDs (e.g., QB=0, RB=2).
    #[serde(rename = "filterSlotIds",  skip_serializing_if = "Option::is_none")]
    filter_slot_ids: Option<Val<Vec<u8>>>,
}

impl Filter {
    /// Set `filterActive`.
    pub fn active(mut self, on: bool) -> Self {
        self.filter_active = Some(Val { value: on });
        self
    }

    /// Optionally set `filterName`.
    pub fn name_opt(mut self, name: Option<String>) -> Self {
        if let Some(n) = name {
            self.filter_name = Some(Val { value: n });
        }
        self
    }

    /// Optionally set `filterSlotIds`.
    pub fn slots_opt(mut self, slots: Option<Vec<u8>>) -> Self {
        if let Some(v) = slots {
            self.filter_slot_ids = Some(Val { value: v });
        }
        self
    }

    /// Serialize into a `HeaderValue` for `x-fantasy-filter`.
    pub fn into_header_value(self) -> Result<HeaderValue, Box<dyn Error + Send + Sync>> {
        let s = serde_json::to_string(&self)?;
        Ok(HeaderValue::from_str(&s)?)
    }
}
