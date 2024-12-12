use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InfoSection {
    pub comment: String,
    #[serde(rename = "filling-rpc-server")]
    pub filling_rpc_server: String,
    #[serde(rename = "filling-tool-version")]
    pub filling_tool_version: String,
    pub lllcversion: String,
    pub source: String,
    pub source_hash: String,
    pub labels: Option<HashMap<usize, String>>,
}
