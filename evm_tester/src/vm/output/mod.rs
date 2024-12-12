use event::Event;
use value::Value;

pub mod event;
pub mod value;

///
/// The compiler test outcome data.
///
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct ExecutionOutput {
    /// The return data values.
    pub return_data: Vec<Value>,
    /// Whether an exception is thrown,
    pub exception: bool,
    /// The emitted events.
    pub events: Vec<Event>,
    pub system_error: Option<(usize, usize)>
}

impl ExecutionOutput {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(return_data: Vec<Value>, exception: bool, events: Vec<Event>, system_error: Option<(usize, usize)>) -> Self {
        Self {
            return_data,
            exception,
            events,
            system_error,
        }
    }
}

impl From<web3::types::U256> for ExecutionOutput {
    fn from(value: web3::types::U256) -> Self {
        Self {
            return_data: vec![value],
            exception: false,
            events: vec![],
            system_error: None,
        }
    }
}

impl From<bool> for ExecutionOutput {
    fn from(value: bool) -> Self {
        let value = if value {
            web3::types::U256::one()
        } else {
            web3::types::U256::zero()
        };
        value.into()
    }
}

impl From<zkevm_tester::compiler_tests::VmSnapshot> for ExecutionOutput {
    fn from(snapshot: zkevm_tester::compiler_tests::VmSnapshot) -> Self {
        let events = snapshot
            .events
            .into_iter()
            .filter(|event| {
                let first_topic = event.topics.first().expect("Always exists");
                let address = crate::utils::bytes32_to_address(first_topic);
                address
                    >= web3::types::Address::from_low_u64_be(
                        zkevm_opcode_defs::ADDRESS_UNRESTRICTED_SPACE,
                    )
            })
            .map(Event::from)
            .collect();

        match snapshot.execution_result {
            zkevm_tester::compiler_tests::VmExecutionResult::Ok(return_data) => {
                let return_data = return_data
                    .chunks(era_compiler_common::BYTE_LENGTH_FIELD)
                    .map(|word| {
                        let value = if word.len() != era_compiler_common::BYTE_LENGTH_FIELD {
                            let mut word_padded = word.to_vec();
                            word_padded.extend(vec![
                                0u8;
                                era_compiler_common::BYTE_LENGTH_FIELD
                                    - word.len()
                            ]);
                            web3::types::U256::from_big_endian(word_padded.as_slice())
                        } else {
                            web3::types::U256::from_big_endian(word)
                        };
                        value
                    })
                    .collect();

                Self {
                    return_data,
                    exception: false,
                    events,
                    system_error: None,
                }
            }
            zkevm_tester::compiler_tests::VmExecutionResult::Revert(return_data) => {
                let return_data: Vec<_> = return_data
                    .chunks(era_compiler_common::BYTE_LENGTH_FIELD)
                    .map(|word| {
                        let value = if word.len() != era_compiler_common::BYTE_LENGTH_FIELD {
                            let mut word_padded = word.to_vec();
                            word_padded.extend(vec![
                                0u8;
                                era_compiler_common::BYTE_LENGTH_FIELD
                                    - word.len()
                            ]);
                            web3::types::U256::from_big_endian(word_padded.as_slice())
                        } else {
                            web3::types::U256::from_big_endian(word)
                        };
                        value
                    })
                    .collect();

                let mut system_error = None;
                if return_data.len() != 0 {
                    let first_result_slot = return_data[0];
                    if first_result_slot > (web3::types::U256::from(1) << web3::types::U256::from(128)) {
                        if first_result_slot < (web3::types::U256::from(1) << web3::types::U256::from(220)) {
                            let first_result_slot = first_result_slot >> web3::types::U256::from(8);
                            let panic = first_result_slot.low_u32();
                
                            if first_result_slot >= (web3::types::U256::from(1) << web3::types::U256::from(200)) {
                                system_error = Some((1, panic as usize));
                            } else {
                                system_error = Some((2, panic as usize));
                            }
                        }
                    }
                }

                Self {
                    return_data,
                    exception: true,
                    events,
                    system_error
                }
            }
            zkevm_tester::compiler_tests::VmExecutionResult::Panic => Self {
                return_data: vec![],
                exception: true,
                events,
                system_error: None
            },
            zkevm_tester::compiler_tests::VmExecutionResult::MostLikelyDidNotFinish { .. } => {
                Self {
                    return_data: vec![],
                    exception: true,
                    events,
                    system_error: None
                }
            }
        }
    }
}