use reqwest::header::HeaderValue;
use serde::Serialize;

#[derive(Serialize)]
pub struct Val<T> {
    pub value: T,
}

#[derive(Serialize, Default)]
pub struct Filter {
    #[serde(rename = "filterActive", skip_serializing_if = "Option::is_none")]
    filter_active: Option<Val<bool>>,

    #[serde(rename = "filterName", skip_serializing_if = "Option::is_none")]
    filter_name: Option<Val<String>>,

    #[serde(rename = "filterSlotIds", skip_serializing_if = "Option::is_none")]
    filter_slot_ids: Option<Val<Vec<u8>>>,
}

impl Filter {
    pub fn active(mut self, on: bool) -> Self {
        self.filter_active = Some(Val { value: on });
        self
    }
    pub fn name_opt(mut self, name: Option<String>) -> Self {
        if let Some(n) = name {
            self.filter_name = Some(Val { value: n });
        }
        self
    }
    pub fn slots_opt(mut self, slots: Option<Vec<u8>>) -> Self {
        if let Some(v) = slots {
            self.filter_slot_ids = Some(Val { value: v });
        }
        self
    }

    pub fn into_header_value(
        self,
    ) -> Result<HeaderValue, Box<dyn std::error::Error + Send + Sync>> {
        let s = serde_json::to_string(&self)?;
        Ok(HeaderValue::from_str(&s)?)
    }
}
