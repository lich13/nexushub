//! Desktop connection DTOs. Credentials are input-only and deliberately not Debug/Serialize.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MachineTarget {
    #[default]
    Local,
    Remote,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteConnectionView {
    pub target: MachineTarget,
    pub revision: u64,
    pub base_url: Option<String>,
    pub configured: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteConnectionCredentials {
    pub revision: u64,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteRevisionRequest {
    pub revision: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteSelectionRequest {
    pub revision: u64,
    pub target: MachineTarget,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteInvokeRequest {
    pub revision: u64,
    pub command: String,
    pub args: Value,
}

pub type RemoteInvokeResponse = Value;
