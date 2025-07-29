use alloy::primitives::*;
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Copy)]
pub struct FieldTo(pub Option<Address>);

impl<'de> Deserialize<'de> for FieldTo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = FieldTo;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("An empty string or correct address")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                let res = if value.is_empty() {
                    None
                } else {
                    Some(value.parse::<Address>().unwrap())
                };

                Ok(FieldTo(res))
            }
        }
        deserializer.deserialize_str(V)
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AccessListItem {
    pub address: Address,
    pub storage_keys: Vec<U256>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationListItem {
    pub chain_id: web3::types::U256,
    pub address: Address,
    pub nonce: web3::types::U256,
    pub v: Option<web3::types::U256>,
    pub r: web3::types::U256,
    pub s: web3::types::U256,
    pub signer: Option<Address>,
    pub y_parity: web3::types::U256,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub data: Bytes,
    pub gas_limit: U256,
    pub gas_price: Option<U256>,
    pub max_fee_per_gas: Option<U256>,
    pub max_priority_fee_per_gas: Option<U256>,
    pub nonce: U256,
    pub secret_key: B256,
    pub to: FieldTo,
    pub sender: Option<Address>,
    pub value: U256,
    pub access_list: Option<Vec<AccessListItem>>,
    pub authorization_list: Option<Vec<AuthorizationListItem>>,
}
