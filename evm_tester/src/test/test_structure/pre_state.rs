use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AccountState {
    pub balance: web3::types::U256,
    pub code: web3::types::Bytes,
    pub nonce: web3::types::U256,
    pub storage: HashMap<web3::types::U256, web3::types::U256>,
}

pub type PreState = HashMap<web3::types::Address, AccountState>;
