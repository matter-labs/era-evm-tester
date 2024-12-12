use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct PostStateIndexes {
    pub data: usize,
    pub gas: usize,
    pub value: usize,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PostState {
    pub indexes: PostStateIndexes,
    pub hash: web3::types::H256,
    pub logs: web3::types::H256,
    pub txbytes: web3::types::Bytes,
    pub expect_exception: Option<String>,
}
