use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Copy)]
pub struct FieldTo(pub Option<web3::types::Address>);

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
                    Some(value.parse::<web3::types::Address>().unwrap())
                };

                Ok(FieldTo(res))
            }
        }
        deserializer.deserialize_str(V)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub data: web3::types::Bytes,
    pub gas_limit: web3::types::U256,
    pub gas_price: Option<web3::types::U256>,
    pub max_fee_per_gas: Option<web3::types::U256>,
    pub max_priority_fee_per_gas: Option<web3::types::U256>,
    pub nonce: web3::types::U256,
    pub secret_key: web3::types::H256,
    pub to: FieldTo,
    pub sender: Option<web3::types::Address>,
    pub value: web3::types::U256,
}
