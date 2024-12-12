use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostStateForCase {
    pub hash: web3::types::H256,
    pub logs: web3::types::H256,
    pub txbytes: web3::types::Bytes,
    pub expect_exception: Option<String>,
}
