use super::value::Value;

///
/// The compiler test outcome event.
///
#[derive(Debug, Clone, serde::Serialize)]
pub struct Event {
    /// The event address.
    address: Option<web3::types::Address>,
    /// The event topics.
    topics: Vec<Value>,
    /// The event values.
    values: Vec<Value>,
}


impl Event {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(
        address: Option<web3::types::Address>,
        topics: Vec<Value>,
        values: Vec<Value>,
    ) -> Self {
        Self {
            address,
            topics,
            values,
        }
    }
}

impl From<zkevm_tester::events::SolidityLikeEvent> for Event {
    fn from(event: zkevm_tester::events::SolidityLikeEvent) -> Self {
        let mut topics: Vec<Value> = event
            .topics
            .into_iter()
            .map(|topic| web3::types::U256::from_big_endian(topic.as_slice()))
            .collect();

        // Event are written by the system contract, and the first topic is the `msg.sender`
        let address = crate::utils::u256_to_address(&topics.remove(0));

        let values: Vec<Value> = event
            .data
            .chunks(era_compiler_common::BYTE_LENGTH_FIELD)
            .map(|word| {
                let value = if word.len() != era_compiler_common::BYTE_LENGTH_FIELD {
                    let mut word_padded = word.to_vec();
                    word_padded.extend(vec![
                        0u8;
                        era_compiler_common::BYTE_LENGTH_FIELD - word.len()
                    ]);
                    web3::types::U256::from_big_endian(word_padded.as_slice())
                } else {
                    web3::types::U256::from_big_endian(word)
                };
                value
            })
            .collect();

        Self {
            address: Some(address),
            topics,
            values,
        }
    }
}

impl PartialEq<Self> for Event {
    fn eq(&self, other: &Self) -> bool {
        if let (Some(address1), Some(address2)) = (self.address, other.address) {
            if address1 != address2 {
                return false;
            }
        };

        if self.topics.len() != other.topics.len() {
            return false;
        }
        if self.values.len() != other.values.len() {
            return false;
        }

        for index in 0..self.topics.len() {
            let (value1, value2) =
                (&self.topics[index], &other.topics[index]);

            if value1 != value2 {
                return false;
            }
        }

        for index in 0..self.values.len() {
            let (value1, value2) =
                (&self.values[index], &other.values[index]);
            
            if value1 != value2 {
                return false;
            }
        }

        true
    }
}