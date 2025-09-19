use reqwest::header::HeaderValue;
use serde::Serialize;

use crate::Result;

/// Wraps ESPN-style `{ "value": ... }`
#[derive(Debug, Serialize)]
pub struct Val<T> {
    pub value: T,
}

/// Rootless filter object for `/players` endpoint.
/// Only set fields will be serialized (thanks to `skip_serializing_if`).
#[derive(Debug, Default, Serialize)]
pub struct PlayersFilter {
    #[serde(rename = "filterActive", skip_serializing_if = "Option::is_none")]
    pub filter_active: Option<Val<bool>>,

    #[serde(rename = "filterName", skip_serializing_if = "Option::is_none")]
    pub filter_name: Option<Val<String>>,

    #[serde(rename = "filterSlotIds", skip_serializing_if = "Option::is_none")]
    pub filter_slot_ids: Option<Val<Vec<u8>>>,

    /// Simple limit (server-side)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// General-purpose helper: any Serialize → JSON → HeaderValue
pub trait IntoHeaderValue {
    fn into_header_value(&self) -> Result<HeaderValue>;
}

impl<T> IntoHeaderValue for T
where
    T: Serialize,
{
    fn into_header_value(&self) -> Result<HeaderValue> {
        let s = serde_json::to_string(self)?;
        Ok(HeaderValue::from_str(&s)?)
    }
}

/// Convenience constructor used by main from CLI args.
pub fn build_players_filter(
    limit: Option<u32>,
    player_name: Option<String>,
    slots: Option<Vec<u8>>,
    include_active: Option<bool>, // if you still want to set filterActive sometimes
) -> PlayersFilter {
    let mut f = PlayersFilter::default();

    if let Some(n) = limit {
        f.limit = Some(n);
    }
    if let Some(name) = player_name {
        f.filter_name = Some(Val { value: name });
    }
    if let Some(slot_ids) = slots {
        f.filter_slot_ids = Some(Val { value: slot_ids });
    }
    if let Some(active) = include_active {
        f.filter_active = Some(Val { value: active });
    }

    f
}
