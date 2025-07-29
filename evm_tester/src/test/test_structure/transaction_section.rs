use crate::test::case::transaction::{AccessListItem, AuthorizationListItem, FieldTo};
use alloy::primitives::*;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TransactionSection {
    pub data: Vec<Bytes>,
    pub gas_limit: Vec<U256>,
    pub gas_price: Option<U256>,
    pub max_fee_per_gas: Option<U256>,
    pub max_priority_fee_per_gas: Option<U256>,
    pub nonce: U256,
    pub secret_key: B256,
    pub to: FieldTo,
    pub sender: Option<Address>,
    pub value: Vec<U256>,
    pub access_lists: Option<Vec<Option<Vec<AccessListItem>>>>,
    pub authorization_list: Option<Vec<AuthorizationListItem>>,
}
