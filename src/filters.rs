//! Serializable filter types for ESPN's `x-fantasy-filter` header.

use reqwest::header::HeaderValue;
use serde::Serialize;
use std::error::Error;

pub trait IntoHeaderValue {
    /// Serialize into a `HeaderValue` for `x-fantasy-filter`.
    fn into_header_value(self) -> Result<HeaderValue, Box<dyn Error + Send + Sync>>;
}

/// Blanket impl for all `Serialize` types.
/// Serializes `self` to JSON and wraps it in a `HeaderValue`.
impl<T> IntoHeaderValue for T
where
    T: Serialize,
{
    fn into_header_value(self) -> Result<HeaderValue, Box<dyn Error + Send + Sync>> {
        let json = serde_json::to_string(&self)?;
        Ok(HeaderValue::from_str(&json)?)
    }
}

/// Generic wrapper `{ "value": T }` used by ESPN filters.
#[derive(Serialize)]
pub struct Val<T> {
    /// The wrapped filter payload.
    pub value: T,
}

/// Flexible filter with optional fields.
#[derive(Serialize, Default)]
pub struct Filter {
    /// Enable filtering; required for other filters to take effect.
    #[serde(rename = "filterActive", skip_serializing_if = "Option::is_none")]
    filter_active: Option<Val<bool>>,
    /// Filter by player last name (substring).
    #[serde(rename = "filterName", skip_serializing_if = "Option::is_none")]
    filter_name: Option<Val<String>>,
    /// Filter by ESPN slot IDs (e.g., QB=0, RB=2).
    #[serde(rename = "filterSlotIds", skip_serializing_if = "Option::is_none")]
    filter_slot_ids: Option<Val<Vec<u8>>>,
    /// Filter by ESPN filterStatuses [all|free|onteam]
    #[serde(rename = "filterStatus", skip_serializing_if = "Option::is_none")]
    filter_status: Option<Val<Vec<String>>>
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

    /// Optionally set `filterStatuses`
    pub fn statuses_opt<S: Into<String>>(mut self, statuses: Option<Vec<S>>) -> Self {
        if let Some(v) = statuses {
            let vv: Vec<String> = v.into_iter().map(Into::into).collect();
            self.filter_status = Some(Val { value: vv });
        }

        self
    }
}

/// Player filter root level
#[derive(Serialize, Default)]
pub struct PlayerFilter {
    pub players: Filter
}
