use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub mod post_state_for_case;
pub mod pre_block;
pub mod transaction;

use alloy::{primitives::*, serde::quantity::vec};
use itertools::Itertools;
use map::hash_set::HashSet;
use pre_block::PreBlock;
use transaction::{transaction_from_tx_section, Transaction};

use crate::{
    test::filler_structure::{AccountFillerStruct, Labels},
    vm::zk_ee::{ZKsyncOS, ZKsyncOSEVMContext, ZKsyncOSTxExecutionResult},
    Filters, Summary,
};

use super::{
    filler_structure::{ExpectStructure, FillerStructure, LabelValue, U256Parsed},
    test_structure::{pre_state::PreState, TestStructure},
};

#[derive(Debug)]
pub struct Case {
    /// The case label.
    pub label: String,
    pub prestate: PreState,
    pub pre_blocks: Vec<PreBlock>,
    pub expected_state: HashMap<Address, AccountFillerStruct>,
    pub expect_exception: bool,
}

fn parse_label(val: &LabelValue) -> Vec<String> {
    match val {
        LabelValue::Number(index) => {
            vec![index.to_string()]
        }
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
        }
        Labels::Multiple(label_values) => {
            for label_value in label_values {
                fill_from_label_value(label_value, indexes);
            }
        }
    }
}

impl Case {
    pub fn from_ethereum_test(
        test_definition: &TestStructure,
        test_filler: &FillerStructure,
        filters: &Filters,
    ) -> Vec<Self> {
        let mut cases = vec![];
        let test_definition = test_definition.state();

        let mut indexes_for_expected_results = vec![];
        // The boolean represents if the expectException flag is set.
        let mut expected_results_states: Vec<(HashMap<Address, AccountFillerStruct>, bool)> =
            vec![];

        for expected_struct in &test_filler.expect {
            let mut indexes_for_struct = (vec![], vec![], vec![]);

            let expected_accounts = ExpectStructure::get_expected_result(&expected_struct.result);
            // TODO: maybe filter only the exceptions that mark it as "invalid".
            let expect_exception = expected_struct
                .expect_exception
                .as_ref()
                .is_some_and(|m| !m.is_empty());
            expected_results_states.push((expected_accounts, expect_exception));

            if let Some(indexes) = expected_struct.indexes.as_ref() {
                fill_indexes_for_expected_states(&indexes.data, &mut indexes_for_struct.0);

                if let Some(gas_indexes) = &indexes.gas {
                    fill_indexes_for_expected_states(gas_indexes, &mut indexes_for_struct.1);
                } else {
                    indexes_for_struct.1.push("-1".to_string());
                }

                if let Some(value_indexes) = &indexes.value {
                    fill_indexes_for_expected_states(value_indexes, &mut indexes_for_struct.2);
                } else {
                    indexes_for_struct.2.push("-1".to_string());
                }
            } else {
                indexes_for_struct = (
                    vec!["-1".to_string()],
                    vec!["-1".to_string()],
                    vec!["-1".to_string()],
                );
            }

            indexes_for_expected_results.push(indexes_for_struct);
        }

        fn is_case_allowed(label: &Option<String>, index: usize, ruleset: &Vec<String>) -> bool {
            ruleset.contains(&"-1".to_string())
                || ruleset.contains(&index.to_string())
                || (label.is_some() && ruleset.contains(label.as_ref().unwrap()))
        }

        let mut case_counter = 0;

        let data_with_access_lists =
            if let Some(access_lists) = &test_definition.transaction.access_lists {
                assert_eq!(access_lists.len(), test_definition.transaction.data.len());
                test_definition
                    .transaction
                    .data
                    .iter()
                    .zip(access_lists)
                    .collect_vec()
            } else {
                test_definition
                    .transaction
                    .data
                    .iter()
                    .zip(std::iter::repeat(&None))
                    .collect_vec()
            };

        for (data_index, (data, access_list)) in data_with_access_lists.into_iter().enumerate() {
            for (gas_limit_index, gas_limit) in
                test_definition.transaction.gas_limit.iter().enumerate()
            {
                for (value_index, value) in test_definition.transaction.value.iter().enumerate() {
                    let case_idx = case_counter;

                    let label = if test_definition._info.labels.is_some() {
                        test_definition
                            ._info
                            .labels
                            .as_ref()
                            .unwrap()
                            .get(&data_index)
                            .cloned()
                    } else {
                        None
                    };

                    // If label is not preset, we use the index
                    let final_label = label.clone().unwrap_or(case_idx.to_string());

                    // Apply label-based filter
                    if !Filters::check_case_label(filters, final_label.as_str()) {
                        case_counter += 1;

                        continue;
                    }

                    let prestate = test_definition.pre.clone();
                    let access_list = access_list.clone();

                    let transaction = transaction_from_tx_section(
                        &test_definition.transaction,
                        *value,
                        data,
                        *gas_limit,
                        access_list,
                    );
                    let pre_block = PreBlock {
                        env: test_definition.env.clone(),
                        transactions: vec![transaction],
                    };

                    let mut expected_state_index: isize = -1;

                    for (idx, index_tuple) in indexes_for_expected_results.iter().enumerate() {
                        if is_case_allowed(&label, data_index, &index_tuple.0)
                            && is_case_allowed(&label, gas_limit_index, &index_tuple.1)
                            && is_case_allowed(&label, value_index, &index_tuple.2)
                        {
                            expected_state_index = idx.try_into().unwrap();
                            break;
                        }
                    }

                    if expected_state_index == -1 {
                        panic!("Not found expected state for case: {case_idx}");
                    }

                    let index: usize = expected_state_index.try_into().unwrap();
                    let (expected_state, expect_exception) = &expected_results_states[index];

                    cases.push(Case {
                        label: final_label,
                        prestate,
                        pre_blocks: vec![pre_block],
                        expected_state: expected_state.clone(),
                        expect_exception: *expect_exception,
                    });

                    case_counter += 1;
                }
            }
        }

        cases
    }

    pub fn from_ethereum_spec_test(
        test_definition: &TestStructure,
        filters: &Filters,
        hardfork_version: &str,
    ) -> Vec<Self> {
        let mut cases = vec![];
        let test_definition = test_definition.state();

        let mut indexes_for_expected_results = vec![];
        // The boolean represents if the expectException flag is set.
        let mut expected_results_states: Vec<(HashMap<Address, AccountFillerStruct>, bool)> =
            vec![];

        let post_state_structs = test_definition.post.get(hardfork_version);

        if post_state_structs.is_none() {
            // Don't have tests for current hardfork
            return vec![];
        }

        for post_state in post_state_structs.unwrap() {
            let mut indexes_for_struct = (vec![], vec![], vec![]);

            if post_state.state.is_none() {
                panic!("Empty state in expected state struct (not filled)");
            }

            let expected_accounts =
                ExpectStructure::get_expected_result(post_state.state.as_ref().unwrap());

            let expect_exception = post_state
                .expect_exception
                .as_ref()
                .is_some_and(|m| !m.is_empty());
            expected_results_states.push((expected_accounts, expect_exception));

            indexes_for_struct
                .0
                .push(post_state.indexes.data.to_string());
            indexes_for_struct
                .1
                .push(post_state.indexes.gas.to_string());
            indexes_for_struct
                .2
                .push(post_state.indexes.value.to_string());

            indexes_for_expected_results.push(indexes_for_struct);
        }

        fn is_case_allowed(label: &Option<String>, index: usize, ruleset: &Vec<String>) -> bool {
            ruleset.contains(&"-1".to_string())
                || ruleset.contains(&index.to_string())
                || (label.is_some() && ruleset.contains(label.as_ref().unwrap()))
        }

        let data_with_access_lists =
            if let Some(access_lists) = &test_definition.transaction.access_lists {
                assert_eq!(access_lists.len(), test_definition.transaction.data.len());
                test_definition
                    .transaction
                    .data
                    .iter()
                    .zip(access_lists)
                    .collect_vec()
            } else {
                test_definition
                    .transaction
                    .data
                    .iter()
                    .zip(std::iter::repeat(&None))
                    .collect_vec()
            };
        let mut case_counter = 0;
        for (data_index, (data, access_list)) in data_with_access_lists.into_iter().enumerate() {
            for (gas_limit_index, gas_limit) in
                test_definition.transaction.gas_limit.iter().enumerate()
            {
                for (value_index, value) in test_definition.transaction.value.iter().enumerate() {
                    let case_idx = case_counter;

                    let label = if test_definition._info.labels.is_some() {
                        test_definition
                            ._info
                            .labels
                            .as_ref()
                            .unwrap()
                            .get(&data_index)
                            .cloned()
                    } else {
                        None
                    };

                    // If label is not preset, we use the index
                    let final_label = label.clone().unwrap_or(case_idx.to_string());

                    // Apply label-based filter
                    if !Filters::check_case_label(filters, final_label.as_str()) {
                        case_counter += 1;

                        continue;
                    }

                    let prestate = test_definition.pre.clone();

                    let transaction = transaction_from_tx_section(
                        &test_definition.transaction,
                        *value,
                        data,
                        *gas_limit,
                        access_list.clone(),
                    );
                    let pre_block = PreBlock {
                        env: test_definition.env.clone(),
                        transactions: vec![transaction],
                    };

                    let mut expected_state_index: isize = -1;

                    for (idx, index_tuple) in indexes_for_expected_results.iter().enumerate() {
                        if is_case_allowed(&label, data_index, &index_tuple.0)
                            && is_case_allowed(&label, gas_limit_index, &index_tuple.1)
                            && is_case_allowed(&label, value_index, &index_tuple.2)
                        {
                            expected_state_index = idx.try_into().unwrap();
                            break;
                        }
                    }

                    if expected_state_index == -1 {
                        panic!("Not found expected state for case: {case_idx}");
                    }

                    let index: usize = expected_state_index.try_into().unwrap();
                    let (expected_state, expect_exception) = &expected_results_states[index];

                    cases.push(Case {
                        label: final_label,
                        prestate,
                        pre_blocks: vec![pre_block],
                        expected_state: expected_state.clone(),
                        expect_exception: *expect_exception,
                    });

                    case_counter += 1;
                }
            }
        }

        cases
    }

    ///
    /// Runs the case on ZKsync OS.
    ///
    pub fn run_zksync_os(
        self,
        summary: Arc<Mutex<Summary>>,
        vm: ZKsyncOS,
        test_name: String,
        bench: bool,
    ) {
        let name = self.label.clone();
        let result = std::panic::catch_unwind(|| {
            self.run_zksync_os_inner(summary.clone(), vm, test_name.clone(), bench)
        });
        if let Err(e) = result {
            Summary::panicked(summary, format!("{test_name}: {name}"), format!("{:?}", e))
        }
    }

    fn run_zksync_os_inner(
        self,
        summary: Arc<Mutex<Summary>>,
        mut vm: ZKsyncOS,
        test_name: String,
        bench: bool,
    ) {
        let name = self.label;

        // Populate prestate
        for (address, state) in self.prestate {
            vm.set_balance(address, state.balance);

            vm.set_nonce(address, state.nonce);

            if state.code.0.len() > 0 {
                vm.set_predeployed_evm_contract(address, state.code, state.nonce);
            }

            state
                .storage
                .into_iter()
                .for_each(|(storage_key, storage_value)| {
                    vm.set_storage_slot(
                        address,
                        storage_key,
                        B256::from(storage_value.to_be_bytes()),
                    );
                });
        }
        // Collect coinbase and sender address to filter out in balance check
        let mut coinbase_and_sender_addresses = HashSet::new();
        self.pre_blocks.iter().for_each(|pre_block| {
            coinbase_and_sender_addresses.insert(pre_block.env.current_coinbase);
            pre_block.transactions.iter().for_each(|tx| {
                coinbase_and_sender_addresses.insert(tx.common().sender.unwrap());
            })
        });

        let run_result = Self::run_zksync_os_blocks(self.pre_blocks, &mut vm);

        let mut check_successful = true;
        let mut expected: Option<String> = None;
        let mut actual: Option<String> = None;

        // TODO merge with prestate!
        for (address, filler_struct) in self.expected_state {
            if filler_struct.balance.is_some() {
                // We do not have equivalent gas refunds, so balances will be different
                if !coinbase_and_sender_addresses.contains(&address) {
                    let expected_balance = filler_struct.balance.as_ref().unwrap();
                    if let Some(expected_balance_value) = expected_balance.as_value() {
                        if vm.get_balance(address) != expected_balance_value {
                            expected = Some(format!(
                                "Balance of {address:?}: {:?}",
                                expected_balance_value
                            ));
                            actual = Some(vm.get_balance(address).to_string());
                            check_successful = false;
                            break;
                        }
                    }
                }
            }
            if filler_struct.nonce.is_some() {
                let expected_nonce = filler_struct.nonce.as_ref().unwrap();
                if let Some(expected_nonce_value) = expected_nonce.as_value() {
                    if vm.get_nonce(address) != expected_nonce_value {
                        expected =
                            Some(format!("Nonce of {address:?}: {:?}", expected_nonce_value));
                        actual = Some(vm.get_nonce(address).to_string());
                        check_successful = false;
                        break;
                    }
                }
            }

            if filler_struct.code.is_some() {
                let actual_code = vm.get_code(address).unwrap_or_default();

                if actual_code != filler_struct.code.as_ref().unwrap().0 .0 {
                    expected = Some(format!("Code of {address:?} is invalid"));
                    actual = None;

                    check_successful = false;
                    break;
                }
            }

            if filler_struct.storage.is_some() {
                let mut has_storage_divergence = false;
                let storage =
                    AccountFillerStruct::parse_storage(filler_struct.storage.as_ref().unwrap());
                for (key, _) in &storage {
                    let key_u256 =
                        U256::from_str_radix(&key.as_value().unwrap().to_string(), 10).unwrap();

                    let expected_value =
                        AccountFillerStruct::get_storage_value(&storage, key).unwrap();
                    let actual_value = vm.get_storage_slot(address, key_u256);

                    match expected_value {
                        U256Parsed::Value(expected_u256) => {
                            let unwrapped_actual_value = actual_value.unwrap_or_default();
                            if unwrapped_actual_value.0 != expected_u256.to_be_bytes() {
                                expected = Some(format!(
                                    "Storage of {address:?}, {:?}: {:?}",
                                    key.as_value().unwrap(),
                                    expected_u256
                                ));
                                actual = Some(format!("{:?}", actual_value));

                                has_storage_divergence = true;
                                break;
                            }
                        }
                        U256Parsed::Any => {
                            if actual_value.is_none() {
                                expected = Some(format!(
                                    "Storage of {address:?}, {:?}: {:?}",
                                    key.as_value().unwrap(),
                                    "Any value"
                                ));
                                actual = Some("None".to_string());

                                has_storage_divergence = true;
                                break;
                            }
                        }
                    };
                }
                if has_storage_divergence {
                    check_successful = false;
                    break;
                }
            }
        }

        if let Ok(res) = run_result {
            // For the test to pass, we need:
            // * successful state changes
            // * expect_exception => exception
            // Note that not all reverting tests have an expected
            // exception declared.
            // TODO: add res.exception
            if check_successful {
                Summary::passed_runtime(summary, format!("{test_name}: {name}"));
            } else {
                Summary::failed(
                    summary,
                    format!("{test_name}: {name}"),
                    // res.exception,
                    expected,
                    actual,
                );
            }
            //}
        } else {
            // Test case was invalid, we check if this was expected
            if self.expect_exception && check_successful {
                Summary::passed_runtime(summary, format!("{test_name}: {name}"));
            } else {
                Summary::invalid(
                    summary,
                    format!("{test_name}: {name}"),
                    run_result.err().unwrap(),
                );
            }
        }
    }

    fn run_zksync_os_blocks(
        pre_blocks: Vec<PreBlock>,
        vm: &mut ZKsyncOS,
    ) -> Result<Vec<Vec<ZKsyncOSTxExecutionResult>>, String> {
        let mut results = vec![];
        for pre_block in pre_blocks.clone() {
            let r = Self::run_zksync_os_block(vm, pre_block)?;
            results.push(r)
        }
        Ok(results)
    }

    fn run_zksync_os_block(
        vm: &mut ZKsyncOS,
        pre_block: PreBlock,
    ) -> Result<Vec<ZKsyncOSTxExecutionResult>, String> {
        let mut system_context = ZKsyncOSEVMContext::default();

        system_context.chain_id = 1;
        system_context.block_number = pre_block.env.current_number.try_into().unwrap();
        system_context.block_timestamp = pre_block.env.current_timestamp.try_into().unwrap();
        system_context.coinbase = pre_block.env.current_coinbase;
        system_context.block_gas_limit = pre_block.env.current_gas_limit;

        if let Some(base_fee) = pre_block.env.current_base_fee {
            system_context.gas_price = base_fee;
        } else if let Some(fee) = pre_block.transactions.first().unwrap().common().gas_price {
            system_context.gas_price = fee
        } else if let Some(fee) = pre_block
            .transactions
            .get(0)
            .unwrap()
            .common()
            .max_fee_per_gas
        {
            system_context.gas_price = fee
        }

        if let Some(base_fee) = pre_block.env.current_base_fee {
            system_context.base_fee = base_fee;
        }

        if let Some(current_difficulty) = pre_block.env.current_difficulty {
            system_context.block_difficulty = B256::from(current_difficulty.to_be_bytes());
        }

        if let Some(random) = pre_block.env.current_random {
            system_context.block_difficulty = B256::from(random.to_be_bytes());
        }
        vm.execute_transactions(pre_block.transactions, system_context)
    }
}
