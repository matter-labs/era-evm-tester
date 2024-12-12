use serde::Deserialize;

use crate::test::case::transaction::FieldTo;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TransactionSection {
    pub data: Vec<web3::types::Bytes>,
    pub gas_limit: Vec<web3::types::U256>,
    pub gas_price: Option<web3::types::U256>,
    pub max_fee_per_gas: Option<web3::types::U256>,
    pub max_priority_fee_per_gas: Option<web3::types::U256>,
    pub nonce: web3::types::U256,
    pub secret_key: web3::types::H256,
    pub to: FieldTo,
    pub sender: Option<web3::types::Address>,
    pub value: Vec<web3::types::U256>,
}
