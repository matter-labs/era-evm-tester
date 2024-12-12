use std::{borrow::Borrow, collections::HashMap, sync::{Arc, Mutex}};

pub mod transaction;
pub mod post_state_for_case;

use post_state_for_case::PostStateForCase;
use revm::{db::{states::plain_account, PlainAccount}, primitives::AccountInfo};
use revme::cmd::statetest::merkle_trie::state_merkle_trie_root;
use transaction::Transaction;
use zksync_types::system_context;

use crate::{test::filler_structure::{AccountFillerStruct, Labels}, utils, vm::eravm::system_context::SystemContext, EraVM, EraVMDeployer, Summary};

use super::{filler_structure::{ExpectStructure, FillerStructure, LabelValue, U256Parsed}, test_structure::{env_section::EnvSection, pre_state::PreState, TestStructure}};

#[derive(Debug)]
pub struct Case {
    /// The case label.
    pub label: String,
    pub prestate: PreState,
    pub transaction: Transaction,
    pub post_state: Option<PostStateForCase>,
    pub expected_state: HashMap<web3::types::Address, AccountFillerStruct>,
    pub env: EnvSection
}

const EVM_VERSION: &'static str = "Cancun";

fn parse_label(val: &LabelValue) -> Vec<String> {
    match val {
        LabelValue::Number(index) => {
            vec![index.to_string()]
        },
        LabelValue::String(str) => {
            if let Some(label) = str.strip_prefix(":label ") {
                // :label foo bar
                vec![label.to_string()]
            } else {
                // x-y
                let range: Vec<_> = str.split("-").map(|x| x.to_string()).collect();

                let range_start = range[0].parse::<usize>().unwrap();
                let range_end = range[1].parse::<usize>().unwrap();

                let mut res = vec![];
                for i in range_start..=range_end {
                    res.push(i.to_string());
                }

                res
            }
        }
    }

}

fn fill_from_label_value(label_value: &LabelValue, indexes: &mut Vec<String>) {
    let labels = parse_label(label_value);
    indexes.extend(labels);
}

fn fill_indexes_for_expected_states(labels: &Labels, indexes: &mut Vec<String>) {
    match labels {
        Labels::Single(label_value) => {
            fill_from_label_value(label_value, indexes);
        },
        Labels::Multiple(label_values) => {
            for label_value in label_values {
                fill_from_label_value(label_value, indexes);
            }
        },
    }
}

impl Case {
    pub fn from_ethereum_test(test_definition: &TestStructure, test_filler: &FillerStructure) -> Vec<Self> {
        let mut cases = vec![];

        let mut indexes_for_expected_results = vec![];
        let mut expected_results_states: Vec<HashMap<zksync_types::H160, AccountFillerStruct>> = vec![];
        
        for expected_struct in &test_filler.expect {
            let mut indexes_for_struct = (vec![], vec![], vec![]);

            let expected_accounts = ExpectStructure::get_expected_result(&expected_struct.result);
            expected_results_states.push(expected_accounts);

            if let Some(indexes) =  expected_struct.indexes.as_ref() {
                fill_indexes_for_expected_states(&indexes.data, &mut indexes_for_struct.0);

                if let Some(gas_indexes) = &indexes.gas {
                    fill_indexes_for_expected_states(&gas_indexes, &mut indexes_for_struct.1);
                } else {
                    indexes_for_struct.1.push("-1".to_string());
                }

                if let Some(value_indexes) = &indexes.value {
                    fill_indexes_for_expected_states(&value_indexes, &mut indexes_for_struct.2);
                } else {
                    indexes_for_struct.2.push("-1".to_string());
                }
            } else {
                indexes_for_struct = (vec!["-1".to_string()], vec!["-1".to_string()], vec!["-1".to_string()]);
            }

            indexes_for_expected_results.push(indexes_for_struct);
        }

        fn is_case_allowed(label: &Option<String>, index: usize, ruleset: &Vec<String>) -> bool {
            ruleset.contains(&"-1".to_string()) || ruleset.contains(&index.to_string()) ||
            (label.is_some() && ruleset.contains(label.as_ref().unwrap()))
        }

        let mut case_counter = 0;
        for (data_index, data) in test_definition.transaction.data.iter().enumerate() {
            for (gas_limit_index, gas_limit) in test_definition.transaction.gas_limit.iter().enumerate() {
                for (value_index, value) in test_definition.transaction.value.iter().enumerate() {
                    let case_idx = case_counter;

                    let label = if test_definition._info.labels.is_some() {
                        test_definition._info.labels.as_ref().unwrap().get(&data_index).cloned()
                    } else {
                        None
                    };

                    let prestate = test_definition.pre.clone();

                    let transaction = Transaction {
                        data: data.clone(),
                        gas_limit: *gas_limit,
                        gas_price: test_definition.transaction.gas_price,
                        nonce: test_definition.transaction.nonce,
                        secret_key: test_definition.transaction.secret_key,
                        to: test_definition.transaction.to,
                        sender: test_definition.transaction.sender,
                        value: *value,
                        max_fee_per_gas: test_definition.transaction.max_fee_per_gas,
                        max_priority_fee_per_gas: test_definition.transaction.max_priority_fee_per_gas,
                    };


                    /*let post_state_for_case = PostStateForCase {
                        hash: expected_result.hash,
                        logs: expected_result.logs,
                        txbytes: expected_result.txbytes.clone(),
                        expect_exception: expected_result.expect_exception.clone(),
                    };*/

                    let mut expected_state_index: isize = -1;

                    for (idx, index_tuple) in indexes_for_expected_results.iter().enumerate() {
                        if is_case_allowed(&label, data_index, &index_tuple.0) 
                        && is_case_allowed(&label, gas_limit_index, &index_tuple.1) 
                        && is_case_allowed(&label, value_index, &index_tuple.2) {
                            expected_state_index = idx.try_into().unwrap();
                            break;
                        }
                    }
        
                    if expected_state_index == -1 {
                        panic!("Not found expected state for case: {case_idx}");
                    }
        
                    let index: usize = expected_state_index.try_into().unwrap();
                    let expected_state = &expected_results_states[index];

                    cases.push(Case {
                        label: label.unwrap_or(case_idx.to_string()),
                        prestate,
                        transaction,
                        post_state: None,
                        expected_state: expected_state.clone(),
                        env: test_definition.env.clone()
                    });

                    case_counter = case_counter + 1;
                }
            }
        }

        cases
    }

    ///
    /// Runs the case on EVM interpreter.
    ///
    pub fn run_evm_interpreter<D, const M: bool>(
        self,
        summary: Arc<Mutex<Summary>>,
        mut vm: EraVM,
        test_name: String,
        test_group: Option<String>,
    ) where
        D: EraVMDeployer,
    {
        let name = self.label;
        
        // Populate prestate
        for (address, state) in self.prestate {
            vm.set_balance(address, state.balance);

            vm.set_nonce(address, state.nonce);

            vm.set_predeployed_evm_contract(address, state.code.0);

            vm.populate_storage(
                state.storage.into_iter().map(|(storage_key, storage_value)| {
                    ((address,  storage_key), utils::u256_to_h256(&storage_value))
                }).collect()
            );
        }

        let mut system_context = SystemContext::default_context(era_compiler_common::Target::EVM);

        system_context.block_number = self.env.current_number.try_into().unwrap();
        system_context.block_timestamp = self.env.current_timestamp.try_into().unwrap();
        system_context.coinbase = self.env.current_coinbase;
        system_context.block_gas_limit = self.env.current_gas_limit;

        if let Some(gas_price) = self.transaction.gas_price {
            system_context.gas_price = gas_price;
        } else {
            if let Some(base_fee) = self.env.current_base_fee {
                let mut gas_price = base_fee;

                if let Some(max_priority_fee) = self.transaction.max_priority_fee_per_gas {
                    gas_price = gas_price + max_priority_fee;
                }

                system_context.gas_price = gas_price;
            }
        }

        if let Some(base_fee) = self.env.current_base_fee {
            system_context.base_fee = base_fee;
        }

        if let Some(current_difficulty) = self.env.current_difficulty {
            system_context.block_difficulty = utils::u256_to_h256(&current_difficulty);
        }

        if let Some(random) = self.env.current_random {
            system_context.block_difficulty = utils::u256_to_h256(&random);
        }

        let run_result = if self.transaction.to.0.is_none() {
            vm.deploy_evm::<M>(
                name.clone(), 
                self.transaction.sender.unwrap(), 
                self.transaction.data.0.clone(), 
                Some(self.transaction.value.as_u128()),
                Some(self.transaction.gas_limit),
                Some(system_context)
            )
        } else {
            vm.execute_evm_interpreter::<M>(
                name.clone(), 
                self.transaction.to.0.unwrap(), // TODO deploy tx
                self.transaction.sender.unwrap(), // TODO derive sender 
                Some(self.transaction.value.as_u128()), // TODO check overflow
                Some(self.transaction.gas_limit),
                self.transaction.data.0.clone(), 
                None,
                Some(system_context)
            )
        };

        let mut check_successful = true;
        let mut expected: Option<String> = None;
        let mut actual: Option<String> = None;
        // TODO merge with prestate!
        for (address, filler_struct) in self.expected_state {
            if filler_struct.balance.is_some() {
                let expected_balance = filler_struct.balance.as_ref().unwrap();
                if let Some(expected_balance_value) = expected_balance.as_value() {
                    if vm.get_balance(address) != expected_balance_value {
                        expected = Some(format!("Balance of {address:?}: {:?}", expected_balance_value));
                        actual = Some(vm.get_balance(address).to_string());
                        check_successful = false;
                        break;
                    }
                }
            }

            if filler_struct.nonce.is_some() {
                let expected_nonce = filler_struct.nonce.as_ref().unwrap();
                if let Some(expected_nonce_value) = expected_nonce.as_value() {
                    if vm.get_nonce(address) != expected_nonce_value {
                        expected = Some(format!("Nonce of {address:?}: {:?}", expected_nonce_value));
                        actual = Some(vm.get_nonce(address).to_string());
                        check_successful = false;
                        break;
                    }
                }
            }

            if filler_struct.code.is_some() {
                let actual_code = if let Some(code) = vm.get_code(address) {
                    code
                } else {
                    Default::default()
                };

                if actual_code != filler_struct.code.unwrap().0.0 {
                    expected = Some(format!("Code of {address:?} is invalid"));
                    actual = None;

                    check_successful = false;
                    break;
                }
            }

            if filler_struct.storage.is_some() {
                let mut has_storage_divergence = false;
                let storage = AccountFillerStruct::parse_storage(filler_struct.storage.as_ref().unwrap());
                for (key, _) in &storage {
                    let key_u256 = web3::types::U256::from_str_radix(&key.as_value().unwrap().to_string(), 10).unwrap();

                    let expected_value = AccountFillerStruct::get_storage_value(&storage, key).unwrap();
                    let actual_value = vm.get_storage_slot(address, key_u256);

                    match expected_value {
                        U256Parsed::Value(expected_u256) => {
                            let unwrapped_actual_value = actual_value.unwrap_or_default(); // TODO check tests logic
                            if unwrapped_actual_value != utils::u256_to_h256(&expected_u256) {
                                expected = Some(format!("Storage of {address:?}, {:?}: {:?}", key.as_value().unwrap(), utils::u256_to_h256(&expected_u256)));
                                actual = Some(format!("{:?}", actual_value));
        
                                has_storage_divergence = true;
                                break;
                            }
                        },
                        U256Parsed::Any => {
                            if actual_value.is_none() {
                                expected = Some(format!("Storage of {address:?}, {:?}: {:?}", key.as_value().unwrap(), "Any value"));
                                actual = Some(format!("None"));
        
                                has_storage_divergence = true;
                                break;
                            }
                        },
                    };

                }
                if has_storage_divergence {
                    check_successful = false;
                    break;
                }
            }
        }

        if let Ok(res) = run_result {
            //println!("GAS USED: {:?}", res.gas);
            if let Some(system_error) = res.output.system_error {
                match system_error.0 {
                    1 => {
                        // forbidden opcode
                        //println!("{test_name}: {name}: FORBIDDEN OPCODE: {:#0x}", system_error.1)
                    },
                    2 => {
                        // forbidden precompile
                        //println!("{test_name}: {name}: FORBIDDEN PRECOMPILE: {:#0x}", system_error.1)

                    },
                    _ => {panic!("Invalid system error type: {:?}", system_error)}
                }

                Summary::ignored(summary, name);
                return;
            }
            /*if res.output.exception {
                Summary::failed(
                    summary, 
                    format!("{test_name}: {name}"), 
                    Some("Finish successfully".to_string()),
                    Some("Failed with exception".to_string()),
                    self.transaction.data.0
                );
            } else {*/
                if check_successful {
                    Summary::passed_runtime(
                        summary,
                        format!("{test_name}: {name}"),
                        test_group,
                        res.cycles,
                        res.ergs,
                        res.gas,
                    );
    
                } else {
                    Summary::failed(
                        summary, 
                        format!("{test_name}: {name}"),
                        res.output.exception,
                        expected,
                        actual,
                        self.transaction.data.0
                    );
                }
            //}
        } else {
            Summary::invalid(
                summary, 
                format!("{test_name}: {name}"), 
                run_result.err().unwrap(),
                self.transaction.data.0
            );
        }
    }
}