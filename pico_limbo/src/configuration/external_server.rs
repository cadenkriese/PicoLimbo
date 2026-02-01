use crate::configuration::require_boolean::{require_false, require_true};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExternalServerConfig {
    Enabled(EnabledExternalServerConfig),
    Disabled(DisabledExternalServerConfig),
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnabledExternalServerConfig {
    #[serde(deserialize_with = "require_true")]
    enabled: bool,
    pub management_port: i32,
    pub management_secret: String,
}

impl Default for ExternalServerConfig {
    fn default() -> Self {
        Self::Enabled(EnabledExternalServerConfig {
            enabled: true,
            management_port: 25566,
            management_secret: "default_secret".to_string(),
        })
    }
}

#[derive(Deserialize, Serialize)]
pub struct DisabledExternalServerConfig {
    #[serde(deserialize_with = "require_false")]
    enabled: bool,
}
