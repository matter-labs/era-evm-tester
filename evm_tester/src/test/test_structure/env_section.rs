use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EnvSection {
    pub current_coinbase: web3::types::Address,
    pub current_difficulty: Option<web3::types::U256>,
    pub current_random: Option<web3::types::U256>,
    pub current_base_fee: Option<web3::types::U256>,
    pub current_gas_limit: web3::types::U256,
    pub current_number: web3::types::U256,
    pub current_timestamp: web3::types::U256,
    pub previous_hash: Option<web3::types::H256>,
}
