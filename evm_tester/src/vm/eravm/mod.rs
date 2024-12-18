//!
//! The EraVM interface.
//!

pub mod address_iterator;
pub mod address_iterator_evm;
pub mod constants;
pub mod deployers;
pub mod system_context;
pub mod system_contracts;
pub mod evm_bytecode_hash;

#[cfg(feature = "vm2")]
mod vm2_adapter;

use address_iterator::EraVMAddressIterator;
use era_compiler_common::EVMVersion;
use std::collections::HashMap;
use std::ops::Add;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use system_context::EVMContext;
use system_contracts::SYSTEM_CONTRACT_LIST;
use web3::signing::keccak256;
use zkevm_tester::compiler_tests::StorageKey;
use zksync_types::Address;
use zksync_types::H256;
use zksync_types::U256;

use constants::SYSTEM_CALL_BIT;
use zkevm_opcode_defs::ADDRESS_CONTRACT_DEPLOYER;

use crate::utils;
use crate::vm::execution_result::ExecutionResult;

use self::system_context::SystemContext;
use self::system_contracts::SystemContracts;
use self::system_contracts::ADDRESS_EVM_GAS_MANAGER;

use super::output::ExecutionOutput;

#[derive(Debug)]
pub struct EvmAccount {
    pub balance: U256,
    pub nonce: U256,
    pub code: Vec<u8>,
    pub code_hash: H256,
    pub storage: HashMap<U256, U256>,
}
///
/// The EraVM interface.
///
#[derive(Clone)]
pub struct EraVM {
    /// The known contracts.
    known_contracts: HashMap<web3::types::U256, Vec<u8>>,
    /// The default account abstraction contract code hash.
    default_aa_code_hash: web3::types::U256,
    /// The EVM interpreter contract code hash.
    evm_interpreter_code_hash: web3::types::U256,
    /// The deployed contracts.
    deployed_contracts: HashMap<web3::types::Address, Vec<u8>>,
    /// The published EVM bytecodes
    published_evm_bytecodes: HashMap<web3::types::U256, Vec<web3::types::U256>>,
    /// The storage state.
    storage: HashMap<zkevm_tester::compiler_tests::StorageKey, web3::types::H256>,
    /// The transient storage state.
    storage_transient: HashMap<zkevm_tester::compiler_tests::StorageKey, web3::types::H256>,
    /// The current EVM block number.
    current_evm_block_number: u128,
    /// The target instruction set.
    _target: era_compiler_common::Target,
    active_addresses: Vec<Address>,
    evm_bytecodes: HashMap<Address, (Vec<u8>, H256)>,
    _address_iterator: EraVMAddressIterator,
    system_context: EVMContext,
}

impl EraVM {
    /// The default address of the benchmark caller.
    pub const DEFAULT_BENCHMARK_CALLER_ADDRESS: &'static str =
        "eeaffc9ff130f15d470945fd04b9017779c95dbf";

    /// The extra amount of gas consumed by every call to the EVM interpreter.
    pub const EVM_INTERPRETER_GAS_OVERHEAD: u64 = 2500;

    /// The `allowedBytecodesToDeploy` variable storage slot in the `ContractDeployer` contract.
    pub const CONTRACT_DEPLOYER_ALLOWED_BYTECODES_MODE_SLOT: &'static str =
        "0xd70708d0b933e26eab552567ce3a8ad69e6fbec9a2a68f16d51bd417a47d9d3b";

    pub const CONTRACT_DEPLOYER_EVM_HASH_PREFIX_SHIFT: u64 = 254;

    /// The `passGas` variable transient storage slot in the `EvmGasManager` contract.
    pub const EVM_GAS_MANAGER_GAS_TRANSIENT_SLOT: u64 = 4;

    /// The `auxData` variable transient storage slot in the `EvmGasManager` contract.
    pub const EVM_GAS_MANAGER_AUX_DATA_TRANSIENT_SLOT: u64 = 5;

    /// The EVM call gas limit.
    pub const EVM_CALL_GAS_LIMIT: u64 = u32::MAX as u64;

    ///
    /// Creates and initializes a new EraVM instance.
    ///
    pub fn new(target: era_compiler_common::Target) -> anyhow::Result<Self> {
        let system_contracts = SystemContracts::build()?;

        let mut storage = SystemContext::create_storage(target);
        let storage_transient = HashMap::new();

        let default_system_context = SystemContext::default_context(target);
        SystemContext::set_system_context(&mut storage, &default_system_context);

        // TODO move to the SystemContext after EVM emulator is ready
        storage.insert(
            zkevm_tester::compiler_tests::StorageKey {
                address: web3::types::Address::from_low_u64_be(ADDRESS_CONTRACT_DEPLOYER.into()),
                key: web3::types::U256::from(Self::CONTRACT_DEPLOYER_ALLOWED_BYTECODES_MODE_SLOT),
            },
            web3::types::H256::from_low_u64_be(1), // Allow EVM contracts deployment
        );

        let mut vm = Self {
            known_contracts: HashMap::new(),
            default_aa_code_hash: web3::types::U256::from_big_endian(
                system_contracts
                    .default_aa
                    .bytecode_hash
                    .expect("Always exists")
                    .as_slice(),
            ),
            evm_interpreter_code_hash: web3::types::U256::from_big_endian(
                system_contracts
                    .evm_emulator
                    .bytecode_hash
                    .expect("Always exists")
                    .as_slice(),
            ),
            deployed_contracts: HashMap::new(),
            storage,
            storage_transient,
            published_evm_bytecodes: HashMap::new(),
            current_evm_block_number: SystemContext::INITIAL_BLOCK_NUMBER,
            _target: target,
            active_addresses: vec![],
            evm_bytecodes: Default::default(),
            _address_iterator: EraVMAddressIterator::new(),
            system_context: default_system_context,
        };

        vm.add_known_contract(
            system_contracts.default_aa.bytecode,
            web3::types::U256::from_big_endian(
                system_contracts
                    .default_aa
                    .bytecode_hash
                    .expect("Always exists")
                    .as_slice(),
            ),
        );
        vm.add_known_contract(
            system_contracts.evm_emulator.bytecode,
            web3::types::U256::from_big_endian(
                system_contracts
                    .evm_emulator
                    .bytecode_hash
                    .expect("Always exists")
                    .as_slice(),
            ),
        );

        for (address, build) in system_contracts.deployed_contracts {
            //println!("{address:?} {:?}", hex::encode(build.bytecode_hash.expect("Always exists").as_slice()));
            vm.add_deployed_contract(
                address,
                web3::types::U256::from_big_endian(
                    build.bytecode_hash.expect("Always exists").as_slice(),
                ),
                Some(build.bytecode),
            );
        }

        Ok(vm)
    }

    ///
    /// Clones the VM instance from and adds known contracts for a single test run.
    ///
    /// TODO: make copyless when the VM supports it.
    ///
    pub fn clone_with_contracts(
        vm: Arc<Self>,
        known_contracts: HashMap<web3::types::U256, Vec<u8>>,
        _evm_version: Option<EVMVersion>,
    ) -> Self {
        let mut vm_clone = (*vm).clone();
        for (bytecode_hash, bytecode) in known_contracts.into_iter() {
            vm_clone.add_known_contract(bytecode, bytecode_hash);
        }
        vm_clone
        // TODO EVM version
    }

    ///
    /// Sets the given block number as the new current block number in storage.
    ///
    pub fn increment_evm_block_number_and_timestamp(&mut self) {
        let mut system_context_values = vec![(
            web3::types::H256::from_low_u64_be(
                SystemContext::SYSTEM_CONTEXT_VIRTUAL_BLOCK_UPGRADE_INFO_POSITION,
            ),
            web3::types::H256::from_low_u64_be(self.current_evm_block_number as u64),
        )];

        let block_timestamp =
            SystemContext::BLOCK_TIMESTAMP_EVM_STEP * self.current_evm_block_number;

        let block_info_bytes = [
            self.current_evm_block_number.to_be_bytes(),
            block_timestamp.to_be_bytes(),
        ]
        .concat();

        system_context_values.push((
            web3::types::H256::from_low_u64_be(
                SystemContext::SYSTEM_CONTEXT_VIRTUAL_L2_BLOCK_INFO_POSITION,
            ),
            web3::types::H256::from_slice(block_info_bytes.as_slice()),
        ));

        let padded_index = [[0u8; 16], self.current_evm_block_number.to_be_bytes()].concat();
        let padded_slot =
            web3::types::H256::from_low_u64_be(SystemContext::SYSTEM_CONTEXT_BLOCK_HASH_POSITION)
                .to_fixed_bytes()
                .to_vec();
        let key = web3::signing::keccak256([padded_index, padded_slot].concat().as_slice());

        let mut hash = web3::types::U256::from_str(SystemContext::ZERO_BLOCK_HASH)
            .expect("Invalid zero block hash constant");
        hash = hash.add(web3::types::U256::from(self.current_evm_block_number));
        let mut hash_bytes = [0u8; era_compiler_common::BYTE_LENGTH_FIELD];
        hash.to_big_endian(&mut hash_bytes);

        system_context_values.push((
            web3::types::H256::from(key),
            web3::types::H256::from_slice(hash_bytes.as_slice()),
        ));

        for (key, value) in system_context_values {
            self.storage.insert(
                zkevm_tester::compiler_tests::StorageKey {
                    address: web3::types::Address::from_low_u64_be(
                        zkevm_opcode_defs::ADDRESS_SYSTEM_CONTEXT.into(),
                    ),
                    key: web3::types::U256::from_big_endian(key.as_bytes()),
                },
                value,
            );
        }
        self.current_evm_block_number += 1;
    }

    ///
    /// Runs a test transaction.
    ///
    pub fn execute<const M: bool>(
        &mut self,
        test_name: String,
        mut entry_address: web3::types::Address,
        caller: web3::types::Address,
        value: Option<u128>,
        calldata: Vec<u8>,
        system_context: Option<EVMContext>,
        vm_launch_option: Option<zkevm_tester::compiler_tests::VmLaunchOption>,
    ) -> anyhow::Result<ExecutionResult> {
        // TODO cleanup
        let mut context = system_context.unwrap_or(self.system_context.clone());
        context.tx_origin = caller;
        SystemContext::set_system_context(&mut self.storage, &context);
        self.system_context = context;

        let (vm_launch_option, context_u128_value) =
            if let Some(vm_launch_option) = vm_launch_option {
                (vm_launch_option, value)
            } else if M {
                match value {
                    Some(value) => {
                        let r3 = Some(web3::types::U256::from(value));
                        let r4 = Some(web3::types::U256::from_big_endian(entry_address.as_bytes()));
                        let r5 = Some(web3::types::U256::from(u8::from(SYSTEM_CALL_BIT)));

                        entry_address = web3::types::Address::from_low_u64_be(
                            zkevm_opcode_defs::ADDRESS_MSG_VALUE.into(),
                        );

                        let vm_launch_option =
                            zkevm_tester::compiler_tests::VmLaunchOption::ManualCallABI(
                                zkevm_tester::compiler_tests::FullABIParams {
                                    is_constructor: false,
                                    is_system_call: true,
                                    r3_value: r3,
                                    r4_value: r4,
                                    r5_value: r5,
                                },
                            );
                        (vm_launch_option, None)
                    }
                    None => (zkevm_tester::compiler_tests::VmLaunchOption::Default, None),
                }
            } else {
                (zkevm_tester::compiler_tests::VmLaunchOption::Default, value)
            };

        let mut trace_file_path = PathBuf::from_str("./trace/").expect("Always valid");
        let trace_file_name = regex::Regex::new("[^A-Za-z0-9]+")
            .expect("Always valid")
            .replace_all(test_name.as_str(), "_")
            .to_string();
        trace_file_path.push(trace_file_name);

        let context = zkevm_tester::compiler_tests::VmExecutionContext::new(
            entry_address,
            caller,
            context_u128_value.unwrap_or_default(),
            0,
        );

        self.increase_nonce(caller);

        #[cfg(not(feature = "vm2"))]
        {
            let snapshot = zkevm_tester::compiler_tests::run_vm_multi_contracts(
                trace_file_path.to_string_lossy().to_string(),
                self.deployed_contracts.clone(),
                &calldata,
                self.storage.clone(),
                self.storage_transient.clone(),
                entry_address,
                Some(context),
                vm_launch_option,
                usize::MAX,
                self.known_contracts.clone(),
                self.published_evm_bytecodes.clone(),
                self.default_aa_code_hash,
                self.evm_interpreter_code_hash,
            )?;

            for (hash, preimage) in snapshot.published_sha256_blobs.iter() {
                if self.published_evm_bytecodes.contains_key(hash) {
                    continue;
                }

                self.published_evm_bytecodes.insert(*hash, preimage.clone());
            }

            for (address, assembly) in snapshot.deployed_contracts.iter() {
                if self.deployed_contracts.contains_key(address) {
                    continue;
                }

                self.deployed_contracts
                    .insert(*address, assembly.to_owned());

                self.active_addresses.push(*address);
            }

            self.storage.clone_from(&snapshot.storage);

            Ok(snapshot.into())
        }
        #[cfg(feature = "vm2")]
        {
            let (result, storage_changes, deployed_contracts) = vm2_adapter::run_vm(
                self.deployed_contracts.clone(),
                &calldata,
                self.storage.clone(),
                entry_address,
                Some(context),
                vm_launch_option,
                self.known_contracts.clone(),
                self.default_aa_code_hash,
                self.evm_interpreter_code_hash,
            )
            .map_err(|error| anyhow::anyhow!("EraVM failure: {}", error))?;

            for (key, value) in storage_changes.into_iter() {
                self.storage.insert(key, value);
            }
            for (address, assembly) in deployed_contracts.into_iter() {
                if self.deployed_contracts.contains_key(&address) {
                    continue;
                }

                self.deployed_contracts.insert(address, assembly);
            }

            Ok(result)
        }
    }

    pub fn deploy_evm<const M: bool>(
        &mut self,
        test_name: String,
        caller: web3::types::Address,
        constructor_input: Vec<u8>,
        value: Option<u128>,
        gas: Option<web3::types::U256>,
        system_context: Option<EVMContext>,
    ) -> anyhow::Result<ExecutionResult> {
        if constructor_input.len() > 49152 {
            // EIP-3860
            // TODO
            return Ok(ExecutionResult {
                output: ExecutionOutput {
                    return_data: vec![],
                    exception: true,
                    events: vec![],
                    system_error: None,
                },
                cycles: 0,
                ergs: 0,
                gas: U256::zero(),
            });
        }

        let mut gas_limit = if let Some(gas) = gas {
            gas
        } else {
            U256::from(Self::EVM_CALL_GAS_LIMIT)
        };

        let system_context_unwrapped = system_context.unwrap_or(SystemContext::default_context(
            era_compiler_common::Target::EVM,
        ));
        let coinbase = system_context_unwrapped.coinbase;
        let gas_price = system_context_unwrapped.gas_price;
        let res = self.pay_for_gas(caller, coinbase, gas_limit, gas_price);
        if res.is_err() {
            // can't pay for gas
            return Ok(ExecutionResult {
                output: ExecutionOutput {
                    return_data: vec![],
                    exception: true,
                    events: vec![],
                    system_error: None,
                },
                cycles: 0,
                ergs: 0,
                gas: gas_limit,
            });
        }

        if let Some(gas_after_intrisic) =
            Self::charge_intristic_cost_and_calldata(gas_limit, &constructor_input, true)
        {
            gas_limit = gas_after_intrisic;
        } else {
            // out of gas
            return Ok(ExecutionResult {
                output: ExecutionOutput {
                    return_data: vec![],
                    exception: true,
                    events: vec![],
                    system_error: None,
                },
                cycles: 0,
                ergs: 0,
                gas: gas_limit,
            });
        }

        // TODO: pay for gas

        // add initial frame data in EvmGasManager
        // set `passGas` to `EVM_CALL_GAS_LIMIT`
        self.storage_transient.insert(
            zkevm_tester::compiler_tests::StorageKey {
                address: web3::types::Address::from_low_u64_be(ADDRESS_EVM_GAS_MANAGER.into()),
                key: web3::types::U256::from(Self::EVM_GAS_MANAGER_GAS_TRANSIENT_SLOT),
            },
            utils::u256_to_h256(&gas_limit),
        );

        // set `isActiveFrame` to true
        self.storage_transient.insert(
            zkevm_tester::compiler_tests::StorageKey {
                address: web3::types::Address::from_low_u64_be(ADDRESS_EVM_GAS_MANAGER.into()),
                key: web3::types::U256::from(Self::EVM_GAS_MANAGER_AUX_DATA_TRANSIENT_SLOT),
            },
            web3::types::H256::from_low_u64_be(2), // "activeFrame flag"
        );

        // TODO move to deployers?
        let context_u128_value;
        let vm_launch_option;
        let mut entry_address = web3::types::Address::from_low_u64_be(
            zkevm_opcode_defs::ADDRESS_CONTRACT_DEPLOYER.into(),
        );

        if M {
            context_u128_value = 0;

            let mut r3 = None;
            let mut r4 = None;
            let mut r5 = None;
            if let Some(value) = value {
                if value != 0 {
                    let value = web3::types::U256::from(value);

                    r3 = Some(value);
                    r4 = Some(web3::types::U256::from(
                        zkevm_opcode_defs::ADDRESS_CONTRACT_DEPLOYER,
                    ));
                    r5 = Some(web3::types::U256::from(u8::from(SYSTEM_CALL_BIT)));

                    entry_address = web3::types::Address::from_low_u64_be(
                        zkevm_opcode_defs::ADDRESS_MSG_VALUE.into(),
                    );
                }
            }

            vm_launch_option = zkevm_tester::compiler_tests::VmLaunchOption::ManualCallABI(
                zkevm_tester::compiler_tests::FullABIParams {
                    is_constructor: false,
                    is_system_call: true,
                    r3_value: r3,
                    r4_value: r4,
                    r5_value: r5,
                },
            );
        } else {
            if let Some(value) = value {
                context_u128_value = value;
            } else {
                context_u128_value = 0;
            }

            vm_launch_option = zkevm_tester::compiler_tests::VmLaunchOption::ManualCallABI(
                zkevm_tester::compiler_tests::FullABIParams {
                    is_constructor: false,
                    is_system_call: true,
                    r3_value: None,
                    r4_value: None,
                    r5_value: None,
                },
            );
        }

        let mut calldata = Vec::with_capacity(
            era_compiler_common::BYTE_LENGTH_X32
                + era_compiler_common::BYTE_LENGTH_FIELD * 2
                + constructor_input.len(),
        );

        const EVM_CREATE_METHOD_SIGNATURE: &str = "createEVM(bytes)";
        calldata.extend(crate::utils::selector(EVM_CREATE_METHOD_SIGNATURE));
        calldata.extend(
            web3::types::H256::from_low_u64_be(era_compiler_common::BYTE_LENGTH_FIELD as u64)
                .as_bytes(),
        );
        calldata.extend(
            web3::types::H256::from_low_u64_be((constructor_input.len()) as u64).as_bytes(),
        );
        calldata.extend(constructor_input);

        let result = self.execute::<M>(
            test_name,
            entry_address,
            caller,
            Some(context_u128_value),
            calldata,
            Some(system_context_unwrapped),
            Some(vm_launch_option),
        );

        if let Ok(res) = result {
            if res.output.return_data.is_empty() {
                // Out-of-ergs or failed deploy
                return Ok(ExecutionResult {
                    output: ExecutionOutput {
                        return_data: vec![],
                        exception: true,
                        events: vec![],
                        system_error: None,
                    },
                    cycles: 0,
                    ergs: 0,
                    gas: U256::zero(),
                });
            }

            /*if res.output.system_error.is_none() {
                let gas_left = res
                .output
                .return_data
                .remove(0);

                println!("{:?}", utils::u256_to_h256(&gas_left));
                let gas_left: u64 = gas_left.try_into().unwrap();

                res.gas = gas_limit - gas_left;
            };*/

            Ok(res)
        } else {
            result
        }
    }

    ///
    /// Executes a contract simulating EVM to EVM call, which gives the ability to measure the amount of gas used.
    ///
    pub fn execute_evm_interpreter<const M: bool>(
        &mut self,
        test_name: String,
        entry_address: web3::types::Address,
        caller: web3::types::Address,
        value: Option<u128>,
        gas: Option<web3::types::U256>,
        calldata: Vec<u8>,
        vm_launch_option: Option<zkevm_tester::compiler_tests::VmLaunchOption>,
        system_context: Option<EVMContext>,
    ) -> anyhow::Result<ExecutionResult> {
        let mut gas_limit = if let Some(gas) = gas {
            gas
        } else {
            U256::from(Self::EVM_CALL_GAS_LIMIT)
        };

        let system_context_unwrapped = system_context.unwrap_or(SystemContext::default_context(
            era_compiler_common::Target::EVM,
        ));
        let coinbase = system_context_unwrapped.coinbase;
        let gas_price = system_context_unwrapped.gas_price;

        let res = self.pay_for_gas(caller, coinbase, gas_limit, gas_price);
        if res.is_err() {
            // can't pay for gas
            return Ok(ExecutionResult {
                output: ExecutionOutput {
                    return_data: vec![],
                    exception: true,
                    events: vec![],
                    system_error: None,
                },
                cycles: 0,
                ergs: 0,
                gas: gas_limit,
            });
        }

        if let Some(gas_after_intrisic) =
            Self::charge_intristic_cost_and_calldata(gas_limit, &calldata, false)
        {
            gas_limit = gas_after_intrisic;
        } else {
            // out of gas
            return Ok(ExecutionResult {
                output: ExecutionOutput {
                    return_data: vec![],
                    exception: true,
                    events: vec![],
                    system_error: None,
                },
                cycles: 0,
                ergs: 0,
                gas: gas_limit,
            });
        }

        if !self.can_send_value(caller, value) {
            // can't send value
            return Ok(ExecutionResult {
                output: ExecutionOutput {
                    return_data: vec![],
                    exception: true,
                    events: vec![],
                    system_error: None,
                },
                cycles: 0,
                ergs: 0,
                gas: U256::zero(),
            });
        }

        // add initial frame data in EvmGasManager
        // set `passGas` to `EVM_CALL_GAS_LIMIT`
        self.storage_transient.insert(
            zkevm_tester::compiler_tests::StorageKey {
                address: web3::types::Address::from_low_u64_be(ADDRESS_EVM_GAS_MANAGER.into()),
                key: web3::types::U256::from(Self::EVM_GAS_MANAGER_GAS_TRANSIENT_SLOT),
            },
            utils::u256_to_h256(&gas_limit),
        );

        // set `isActiveFrame` to true
        self.storage_transient.insert(
            zkevm_tester::compiler_tests::StorageKey {
                address: web3::types::Address::from_low_u64_be(ADDRESS_EVM_GAS_MANAGER.into()),
                key: web3::types::U256::from(Self::EVM_GAS_MANAGER_AUX_DATA_TRANSIENT_SLOT),
            },
            web3::types::H256::from_low_u64_be(2), // "activeFrame flag"
        );

        let mut result = self.execute::<M>(
            test_name.clone(),
            entry_address,
            caller,
            value,
            calldata,
            Some(system_context_unwrapped),
            vm_launch_option,
        )?;

        if result.output.return_data.is_empty() {
            if self.get_code(entry_address).is_some() {
                anyhow::bail!("Return data is empty");
            } else {
                let refund_amount = gas_limit * gas_price;

                self.refund_gas(caller, coinbase, refund_amount);
            }
        } else if result.output.system_error.is_none() {
            let gas_left = result.output.return_data.remove(0);

            let gas_left: u64 = gas_left.try_into().unwrap();

            result.gas = gas_limit - gas_left;

            let refund_amount = U256::from(gas_left) * gas_price;

            self.refund_gas(caller, coinbase, refund_amount);
        }

        Ok(result)
    }

    fn charge_intristic_cost_and_calldata(
        mut gas: U256,
        calldata: &Vec<u8>,
        is_deploy: bool,
    ) -> Option<U256> {
        let intristic_cost = U256::from(if is_deploy { 53000 } else { 21000 });

        if gas >= intristic_cost {
            gas -= intristic_cost;
        } else {
            return None;
        }

        // simulate calldataprice
        for byte in calldata.iter() {
            let calldata_byte_price = U256::from(if *byte == 0 { 4 } else { 16 });

            if gas < calldata_byte_price {
                return None;
            }

            gas -= calldata_byte_price;
        }

        Some(gas)
    }

    ///
    /// Performs the check for the storage emptiness, that is, if all its values, except for those
    /// related to system contracts and auxiliary data inaccessible by the user code, are zeros.
    ///
    /// Mostly used by the Ethereum tests.
    ///
    pub fn is_storage_empty(&self) -> bool {
        for (key, value) in self.storage.iter() {
            if key.address
                < web3::types::Address::from_low_u64_be(
                    zkevm_opcode_defs::ADDRESS_UNRESTRICTED_SPACE,
                )
            {
                continue;
            }

            if !value.is_zero() {
                return false;
            }
        }

        true
    }

    pub fn get_state(&self) -> HashMap<Address, EvmAccount> {
        // TODO cleanup
        let mut accounts: HashMap<Address, EvmAccount> = Default::default();

        let user_space_storage: HashMap<_, _> = self
            .storage
            .iter()
            .filter(|(key, _)| {
                let mut user_space = true;
                SYSTEM_CONTRACT_LIST.iter().for_each(|system_contract| {
                    if system_contract.2 == key.address {
                        user_space = false;
                    }
                });

                user_space
            })
            .map(|(key, value)| (*key, *value))
            .collect();

        let accounts_storages: HashMap<Address, HashMap<U256, U256>> = self
            .active_addresses
            .iter()
            .map(|address| {
                let mut storage: HashMap<U256, U256> = Default::default();

                user_space_storage
                    .iter()
                    .filter(|(key, _)| key.address == *address)
                    .for_each(|(key, value)| {
                        storage.insert(key.key, utils::h256_to_u256(value));
                    });

                (*address, storage)
            })
            .collect();

        for address in self.active_addresses.clone() {
            let code;
            let code_hash;
            if self.evm_bytecodes.contains_key(&address) {
                (code, code_hash) = self
                    .evm_bytecodes
                    .get(&address)
                    .cloned()
                    .expect("Always exists");
            } else {
                code = Default::default();
                code_hash = H256::from_slice(&keccak256(&[]));

                println!("EMPTY HASH: {}", code_hash);
            }

            let account = EvmAccount {
                balance: self.get_balance(address),
                nonce: self.get_nonce(address),
                code,
                code_hash,
                storage: accounts_storages[&address].clone(),
            };

            accounts.insert(address, account);
        }

        accounts
    }

    ///
    /// Mints some Ether value at the specified address.
    /// Is needed for payable calls simulation.
    ///
    pub fn mint_ether(&mut self, address: web3::types::Address, amount: web3::types::U256) {
        let key = Self::balance_storage_key(address);
        let old_amount = web3::types::U256::from_big_endian(
            self.storage
                .get(&key)
                .cloned()
                .unwrap_or_default()
                .as_bytes(),
        );
        let new_amount = old_amount + amount;
        let new_amount = crate::utils::u256_to_h256(&new_amount);
        self.storage.insert(key, new_amount);
    }

    ///
    /// Burns some Ether value for at specified address.
    ///
    pub fn burn_ether(&mut self, address: web3::types::Address, amount: web3::types::U256) {
        let key = Self::balance_storage_key(address);
        let old_amount = web3::types::U256::from_big_endian(
            self.storage
                .get(&key)
                .cloned()
                .unwrap_or_default()
                .as_bytes(),
        );
        let new_amount = old_amount - amount;
        let new_amount = crate::utils::u256_to_h256(&new_amount);
        self.storage.insert(key, new_amount);
    }

    ///
    /// Returns the balance of the specified address.
    ///
    pub fn get_balance(&self, address: web3::types::Address) -> web3::types::U256 {
        let key = Self::balance_storage_key(address);
        let balance = self.storage.get(&key).copied().unwrap_or_default();
        web3::types::U256::from_big_endian(balance.as_bytes())
    }

    ///
    /// Changes the balance of the specified address.
    ///
    pub fn set_balance(&mut self, address: web3::types::Address, value: web3::types::U256) {
        let key = Self::balance_storage_key(address);
        self.storage.insert(key, utils::u256_to_h256(&value));
    }

    pub fn can_send_value(&self, address: Address, value: Option<u128>) -> bool {
        if let Some(value) = value {
            if self.get_balance(address) < U256::from(value) {
                return false;
            }
        }

        true
    }

    pub fn pay_for_gas(
        &mut self,
        address: web3::types::Address,
        coinbase: web3::types::Address,
        gas_limit: U256,
        gas_price: U256,
    ) -> Result<U256, String> {
        let amount = gas_limit.checked_mul(gas_price);

        if amount.is_none() {
            return Err("Amount calculation overflow".to_string());
        }

        let amount = amount.unwrap();

        let caller_key = Self::balance_storage_key(address);

        let mut caller_balance =
            utils::h256_to_u256(&self.storage.get(&caller_key).copied().unwrap_or_default());

        if caller_balance < amount {
            return Err("Insufficient balance".to_string());
        }

        caller_balance -= amount;

        self.storage
            .insert(caller_key, utils::u256_to_h256(&caller_balance));

        if !self.active_addresses.contains(&coinbase) {
            self.active_addresses.push(coinbase);
        }

        Ok(amount)
    }

    pub fn refund_gas(
        &mut self,
        address: web3::types::Address,
        coinbase: web3::types::Address,
        amount: U256,
    ) {
        let caller_key = Self::balance_storage_key(address);

        let mut caller_balance =
            utils::h256_to_u256(&self.storage.get(&caller_key).copied().unwrap_or_default());

        caller_balance += amount;

        self.storage
            .insert(caller_key, utils::u256_to_h256(&caller_balance));

        if !self.active_addresses.contains(&coinbase) {
            self.active_addresses.push(coinbase);
        }
    }

    ///
    /// Returns the nonce of the specified address.
    ///
    pub fn get_nonce(&self, address: web3::types::Address) -> web3::types::U256 {
        let key = Self::nonce_storage_key(address);
        let nonce = utils::h256_to_u256(&self.storage.get(&key).copied().unwrap_or_default());

        if self.get_code(address).is_some() {
            nonce >> web3::types::U256::from(128)
        } else {
            web3::types::U256::from(nonce.low_u128())
        }
    }

    ///
    /// Changes the nonce of the specified address.
    ///
    pub fn set_nonce(&mut self, address: web3::types::Address, value: web3::types::U256) {
        assert!(
            value < web3::types::U256::from(1) << web3::types::U256::from(128),
            "Nonce is too big"
        );

        let address_h256 = utils::address_to_h256(&address);

        let bytes = [
            address_h256.as_bytes(),
            &[0; era_compiler_common::BYTE_LENGTH_FIELD],
        ]
        .concat();
        let key = web3::signing::keccak256(&bytes).into();
        let storage_key = zkevm_tester::compiler_tests::StorageKey {
            address: web3::types::Address::from_low_u64_be(
                zkevm_opcode_defs::ADDRESS_NONCE_HOLDER.into(),
            ),
            key,
        };

        let new_raw_nonce = value
            .checked_mul(web3::types::U256::from(2).pow(128.into()))
            .unwrap()
            .add(value);
        self.storage
            .insert(storage_key, utils::u256_to_h256(&new_raw_nonce));
    }

    ///
    /// Increases the nonce of the specified address.
    ///
    pub fn increase_nonce(&mut self, address: web3::types::Address) {
        let key = Self::nonce_storage_key(address);
        let mut nonce = utils::h256_to_u256(&self.storage.get(&key).copied().unwrap_or_default());
        nonce = nonce.add(web3::types::U256::from(1));
        self.storage.insert(key, utils::u256_to_h256(&nonce));
    }

    pub fn set_predeployed_evm_contract(
        &mut self,
        address: web3::types::Address,
        bytecode: Vec<u8>,
    ) {
        self.save_evm_bytecode(address, bytecode.clone());
        self.active_addresses.push(address);

        if bytecode.is_empty() {
            return;
        }

        let mut padded_bytecode = bytecode.clone();

        if padded_bytecode.len() % 32 != 0 {
            let padded_len = (padded_bytecode.len() / 32 + 1) * 32;
            padded_bytecode.extend(vec![0; padded_len - padded_bytecode.len()]);
        }

        if (padded_bytecode.len() / 32) % 2 != 1 {
            padded_bytecode.extend(vec![0; 32]);
        }

        let bytecode_hash = evm_bytecode_hash::hash_evm_bytecode(bytecode.len() as u16, &padded_bytecode);

        self.add_known_evm_contract(padded_bytecode.clone(), utils::h256_to_u256(&bytecode_hash));
        self.add_deployed_contract(
            address,
            utils::h256_to_u256(&bytecode_hash),
            Some(padded_bytecode.clone()),
        );

        let evm_hash = keccak256(&bytecode);

        let address_as_uint256 = utils::address_to_h256(&address);
        let storage_slot_encoding = utils::h256_to_u256(&address_as_uint256)
            + (U256::from(1) << U256::from(Self::CONTRACT_DEPLOYER_EVM_HASH_PREFIX_SHIFT));

        self.storage.insert(
            zkevm_tester::compiler_tests::StorageKey {
                address: web3::types::Address::from_low_u64_be(ADDRESS_CONTRACT_DEPLOYER.into()),
                key: storage_slot_encoding,
            },
            H256::from_slice(&evm_hash),
        );
    }

    pub fn add_active_address(&mut self, address: web3::types::Address) {
        self.active_addresses.push(address);
    }

    ///
    /// Adds a known contract.
    ///
    fn add_known_contract(&mut self, bytecode: Vec<u8>, bytecode_hash: web3::types::U256) {
        self.storage.insert(
            zkevm_tester::compiler_tests::StorageKey {
                address: web3::types::Address::from_low_u64_be(
                    zkevm_opcode_defs::ADDRESS_KNOWN_CODES_STORAGE.into(),
                ),
                key: bytecode_hash,
            },
            web3::types::H256::from_low_u64_be(1),
        );
        self.known_contracts.insert(bytecode_hash, bytecode);
    }

    fn add_known_evm_contract(&mut self, _bytecode: Vec<u8>, bytecode_hash: web3::types::U256) {
        self.storage.insert(
            zkevm_tester::compiler_tests::StorageKey {
                address: web3::types::Address::from_low_u64_be(
                    zkevm_opcode_defs::ADDRESS_KNOWN_CODES_STORAGE.into(),
                ),
                key: bytecode_hash,
            },
            web3::types::H256::from_low_u64_be(1),
        );
        //self.published_evm_bytecodes.insert(bytecode_hash, bytecode);
    }

    fn save_evm_bytecode(&mut self, address: Address, bytecode: Vec<u8>) {
        let hash = keccak256(&bytecode);
        let evm_hash = H256::from_slice(&hash);
        self.evm_bytecodes.insert(address, (bytecode, evm_hash));
    }

    pub fn get_contract_versioned_bytecode_hash(&self, address: Address) -> Option<&H256> {
        self.storage.get(&zkevm_tester::compiler_tests::StorageKey {
            address: web3::types::Address::from_low_u64_be(
                zkevm_opcode_defs::ADDRESS_ACCOUNT_CODE_STORAGE.into(),
            ),
            key: web3::types::U256::from_big_endian(address.as_bytes()),
        })
    }

    pub fn get_code(&self, address: Address) -> Option<Vec<u8>> {
        if let Some(bytecode_hash) = self.get_contract_versioned_bytecode_hash(address) {
            let hash_as_bytes = bytecode_hash.as_bytes();
            let bytecode_len = (hash_as_bytes[3] as usize) + 256 * (hash_as_bytes[2] as usize);

            if let Some(bytecode) = self
                .published_evm_bytecodes
                .get(&utils::h256_to_u256(bytecode_hash))
            {
                let mut res_bytecode: Vec<u8> = vec![];
                for word in bytecode.iter() {
                    res_bytecode.extend(utils::u256_to_h256(word).as_bytes());
                }

                let res_bytecode = res_bytecode.into_iter().take(bytecode_len).collect();
                Some(res_bytecode)
            } else {
                None
            }
        } else {
            None
        }
        //let entry = self.evm_bytecodes.get(&address);
        //if let Some((bytecode, _)) = entry {
        //    return Some(bytecode.clone());
        //} else {
        //    let bytecode_hash = self.get_contract_versioned_bytecode_hash(address);
        //}

        //None
    }

    pub fn get_storage_slot(
        &mut self,
        address: Address,
        key: web3::types::U256,
    ) -> Option<web3::types::H256> {
        self.storage.get(&StorageKey { address, key }).cloned()
    }

    ///
    /// Set contract as deployed on `address`. If `assembly` is none - trying to get assembly from known contracts.
    ///
    /// # Panics
    ///
    /// Will panic if some contract already deployed at `address` or `assembly` in none and contract is not found in known contracts.
    ///
    pub fn add_deployed_contract(
        &mut self,
        address: web3::types::Address,
        bytecode_hash: web3::types::U256,
        bytecode: Option<Vec<u8>>,
    ) {
        if self.deployed_contracts.contains_key(&address) {
            self.remove_deployed_contract(address);
        }

        self.storage.insert(
            zkevm_tester::compiler_tests::StorageKey {
                address: web3::types::Address::from_low_u64_be(
                    zkevm_opcode_defs::ADDRESS_ACCOUNT_CODE_STORAGE.into(),
                ),
                key: web3::types::U256::from_big_endian(address.as_bytes()),
            },
            crate::utils::u256_to_h256(&bytecode_hash),
        );
        let bytecode = match bytecode {
            Some(bytecode) => bytecode,
            None => self
                .known_contracts
                .get(&bytecode_hash)
                .expect("Contract not found in known contracts for deploy")
                .clone(),
        };
        self.deployed_contracts.insert(address, bytecode);
    }

    ///
    /// Remove deployed contract.
    ///
    /// # Panics
    ///
    /// Will panic if any contract is not deployed at `address`
    ///
    pub fn remove_deployed_contract(&mut self, address: web3::types::Address) {
        self.storage
            .remove(&zkevm_tester::compiler_tests::StorageKey {
                address: web3::types::Address::from_low_u64_be(
                    zkevm_opcode_defs::ADDRESS_ACCOUNT_CODE_STORAGE.into(),
                ),
                key: web3::types::U256::from_big_endian(address.as_bytes()),
            })
            .expect("Contract not found");
        self.deployed_contracts
            .remove(&address)
            .expect("Contract not found");
    }

    ///
    /// Adds values to storage.
    ///
    pub fn populate_storage(
        &mut self,
        values: HashMap<(web3::types::Address, web3::types::U256), web3::types::H256>,
    ) {
        self.storage.extend(
            values
                .into_iter()
                .map(|((address, key), value)| {
                    (
                        zkevm_tester::compiler_tests::StorageKey { address, key },
                        value,
                    )
                })
                .collect::<HashMap<zkevm_tester::compiler_tests::StorageKey, web3::types::H256>>(),
        );
    }

    ///
    /// Returns known contract size by code_hash, None if not found.
    ///
    pub fn get_contract_size(&self, code_hash: web3::types::U256) -> usize {
        self.known_contracts
            .get(&code_hash)
            .expect("Always exists")
            .len()
    }

    ///
    /// Gets the balance storage key for the specified address.
    ///
    fn balance_storage_key(
        address: web3::types::Address,
    ) -> zkevm_tester::compiler_tests::StorageKey {
        let mut key_preimage = Vec::with_capacity(era_compiler_common::BYTE_LENGTH_FIELD * 2);
        key_preimage.extend(vec![
            0u8;
            era_compiler_common::BYTE_LENGTH_FIELD
                - era_compiler_common::BYTE_LENGTH_ETH_ADDRESS
        ]);
        key_preimage.extend_from_slice(address.as_bytes());
        key_preimage.extend(vec![0u8; era_compiler_common::BYTE_LENGTH_FIELD]);

        let key_string = era_compiler_common::Hash::keccak256(key_preimage.as_slice());
        let key =
            web3::types::U256::from_str(key_string.to_string().as_str()).expect("Always valid");
        zkevm_tester::compiler_tests::StorageKey {
            address: web3::types::Address::from_low_u64_be(
                zkevm_opcode_defs::ADDRESS_ETH_TOKEN.into(),
            ),
            key,
        }
    }

    ///
    /// Gets the nonce storage key for the specified address.
    ///
    fn nonce_storage_key(
        address: web3::types::Address,
    ) -> zkevm_tester::compiler_tests::StorageKey {
        let address_h256 = utils::address_to_h256(&address);
        let bytes = [
            address_h256.as_bytes(),
            &[0; era_compiler_common::BYTE_LENGTH_FIELD],
        ]
        .concat();
        let key = web3::signing::keccak256(&bytes).into();

        zkevm_tester::compiler_tests::StorageKey {
            address: web3::types::Address::from_low_u64_be(
                zkevm_opcode_defs::ADDRESS_NONCE_HOLDER.into(),
            ),
            key,
        }
    }
}
