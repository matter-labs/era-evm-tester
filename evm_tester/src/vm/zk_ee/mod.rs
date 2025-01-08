
use std::alloc::Global;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Context;
use evm::utils::{h256_to_u256, u256_to_h256};
use revm::primitives::ruint;
use revm::primitives::ruint::aliases::B160;
use transaction::{gen_l2_tx, TransactionData};
use web3::ethabi::{encode, Address, Token};
use zk_ee::common_structs::derive_flat_storage_key;
use zk_ee::system::system_trait::errors::InternalError;
use zk_ee::system::ExecutionEnvironmentType;
use zk_ee::utils::Bytes32;
use zk_os_basic_bootloader::bootloader::errors::InvalidTransaction;
use zk_os_basic_system::basic_io_implementer::address_into_special_storage_key;
use zk_os_basic_system::basic_system::simple_growable_storage::TestingTree;
use zk_os_evm_interpreter::utils::evm_bytecode_into_partial_account_data;
use zk_os_forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree, TxListSource};
use zk_os_forward_system::run::{run_batch, BatchContext, BatchOutput, PreimageSource, PreimageType, StorageCommitment, TxOutput};
use zk_os_system_hooks::addresses_constants::{ACCOUNT_PARTIAL_DATA_STORAGE_ADDRESS, BYTECODE_HASH_STORAGE_ADDRESS, NOMINAL_TOKEN_BALANCE_STORAGE_ADDRESS};
use zksync_types::fee::Fee;
use zksync_types::{K256PrivateKey, H256, U256};

mod transaction;

#[derive(Clone, Default)]
pub struct ZkOsEVMContext {
    pub chain_id: u64,
    pub coinbase: web3::types::Address,
    pub block_number: u128,
    pub block_timestamp: u128,
    pub block_gas_limit: web3::types::U256,
    pub block_difficulty: web3::types::H256,
    pub base_fee: web3::types::U256,
    pub gas_price: web3::types::U256,
    pub tx_origin: web3::types::Address,
}

///
/// The VM execution result.
///
#[derive(Debug, Clone, Default)]
pub struct ZkOsExecutionResult {
    /// The VM snapshot execution result.
    pub return_data: Vec<u8>,
    pub exception: bool,
    /// The number of gas used.
    pub gas: web3::types::U256,
    pub address_deployed: Option<Address>
}


///
/// The ZK OS interface.
///
#[derive(Clone)]
pub struct ZkOS {
    tree: InMemoryTree,
    preimage_source: InMemoryPreimageSource,
}

impl ZkOS {
    pub fn new(
    ) -> Self {
        let tree = InMemoryTree {
            storage_tree: TestingTree::new_in(Global),
            cold_storage: HashMap::new(),
        };
        let preimage_source = InMemoryPreimageSource {
            inner: Default::default(),
        };
        Self {
            tree,
            preimage_source,
        }
    }

    pub fn clone(
        vm: Arc<Self>,
    ) -> Self {
        (*vm).clone()
    }

    pub fn execute_transaction(
        &mut self,
        private_key: H256,
        to: web3::types::Address,
        value: Option<U256>,
        calldata: Vec<u8>,
        gas_limit: U256,
        nonce: u32,
        system_context: ZkOsEVMContext,
    ) -> anyhow::Result<ZkOsExecutionResult, String> {
        println!("EXECUTING ZK OS {}", nonce);

        let fee = Fee {
            gas_limit,
            max_fee_per_gas: system_context.gas_price,
            max_priority_fee_per_gas: Default::default(),
            gas_per_pubdata_limit: Default::default(),
        };

        let l2_tx = gen_l2_tx(
            &K256PrivateKey::from_bytes(private_key).expect("Invalid private key"), 
            to, 
            calldata, 
            value.unwrap_or_default(), 
            nonce,
            fee,
            system_context.block_timestamp as u64, 
            37 // TODO: chainId is hardcoded system_context.chain_id as u64
        ).context("Gen l2 tx").unwrap();

        let tx = TransactionData::from(l2_tx);

        let encoded_tx = tx.abi_encode();

        let tx_source = TxListSource {
            transactions: vec![encoded_tx].into(),
        };

        let context = BatchContext {
            //todo: gas
            eip1559_basefee: ruint::Uint::from_str(&system_context.base_fee.to_string()).expect("Invalid basefee"),
            ergs_price: ruint::Uint::from_str(&system_context.gas_price.to_string()).expect("Invalid ergs price"),
            gas_per_pubdata: Default::default(),
            block_number: system_context.block_number as u64,
            timestamp: system_context.block_timestamp as u64,
        };

        let storage_commitment = StorageCommitment {
            root: self.tree.storage_tree.root().clone(),
            next_free_slot: self.tree.storage_tree.next_free_slot,
        };

        let tree = self.tree.clone();
        let preimage_source = self.preimage_source.clone();

        let result = run_batch(
            context,
            storage_commitment,
            tree,
            preimage_source,
            tx_source
        );

        self.apply_batch_execution_result(result)
    }

    fn apply_batch_execution_result(&mut self, batch_execution_result: Result<BatchOutput, InternalError>) -> anyhow::Result<ZkOsExecutionResult, String> {
        match batch_execution_result {
            Ok(result) => {
                for storage_write in result.storage_writes.iter() {
                    self.tree.cold_storage.insert(
                        storage_write.key,
                        storage_write.value,
                    );
                    self.tree.storage_tree.insert(
                        &storage_write.key,
                        &storage_write.value,
                    );
                }

                for (hash, preimage) in result.published_preimages.iter() {
                    self.preimage_source.inner.insert(
                        (
                            PreimageType::Bytecode(ExecutionEnvironmentType::EVM),
                            *hash,
                        ),
                        preimage.clone(),
                    );
                }

                let tx_result = result.tx_results.get(0).expect("Do not have tx output").clone();

                Self::get_transaction_execution_result(tx_result)
            }
            Err(err) => {
                Err(format!("{err:?}"))
            }
        }
    }

    fn get_transaction_execution_result(tx_result: Result<TxOutput, InvalidTransaction>) -> anyhow::Result<ZkOsExecutionResult, String> {
        match tx_result {
            Ok(tx_output) => {
                let mut execution_result = ZkOsExecutionResult::default();

                execution_result.gas = tx_output.gas_used.into();
                // TODO events

                match &tx_output.execution_result {
                    zk_os_forward_system::run::ExecutionResult::Success(execution_output) => {
                        match execution_output {
                            zk_os_forward_system::run::ExecutionOutput::Call(data) => {
                                execution_result.return_data = data.clone();
                            },
                            zk_os_forward_system::run::ExecutionOutput::Create(data, address) => {
                                execution_result.return_data = data.clone();
                                // execution_result.address_deployed
                            },
                        }
                    },
                    zk_os_forward_system::run::ExecutionResult::Revert(vec) => {
                        execution_result.exception = true;
                        execution_result.return_data = vec.clone();
                    },
                }
                Ok(execution_result)                   
            }
            Err(tx_err) => {
                Err(format!("{tx_err:?}"))
            }
        }

        
    }

    ///
    /// Returns the balance of the specified address.
    ///
    pub fn get_balance(&self, address: web3::types::Address) -> web3::types::U256 {
        let address = address_to_b160(address);
        let key = address_into_special_storage_key(&address);
        let flat_key = derive_flat_storage_key(&NOMINAL_TOKEN_BALANCE_STORAGE_ADDRESS, &key);

        let value = self.tree.cold_storage.get(&flat_key);
        if let Some(res) = value {
            h256_to_u256(bytes32_to_h256(*res))
        } else {
            Default::default()
        }
    }

    ///
    /// Changes the balance of the specified address.
    ///
    pub fn set_balance(&mut self, address: web3::types::Address, value: web3::types::U256) {
        let address = address_to_b160(address);
        let key = address_into_special_storage_key(&address);
        let flat_key = derive_flat_storage_key(&NOMINAL_TOKEN_BALANCE_STORAGE_ADDRESS, &key);


        let value = h256_to_bytes32(u256_to_h256(value));
        self.tree.cold_storage.insert(
            flat_key,
            value,
        );
        self.tree.storage_tree.insert(
            &flat_key,
            &value,
        );
    }

    ///
    /// Returns the nonce of the specified address.
    ///
    pub fn get_nonce(&self, address: web3::types::Address) -> web3::types::U256 {
        let address = address_to_b160(address);
        let key = address_into_special_storage_key(&address);
        let flat_key = derive_flat_storage_key(&ACCOUNT_PARTIAL_DATA_STORAGE_ADDRESS, &key);

        let partial_data = self.tree.cold_storage.get(&flat_key);

        match partial_data {
            Some(partial_data) => {
                let nonce = partial_data.as_u64_array_ref()[3];
                nonce.into()
            },
            None => Default::default(),
        }
    }

    ///
    /// Changes the nonce of the specified address.
    ///
    pub fn set_nonce(&mut self, address: web3::types::Address, value: web3::types::U256) {
        let address = address_to_b160(address);
        let key = address_into_special_storage_key(&address);
        let flat_key = derive_flat_storage_key(&ACCOUNT_PARTIAL_DATA_STORAGE_ADDRESS, &key);

        let mut partial_data = match self.tree.cold_storage.get(&flat_key) {
            Some(partial_data) => {
                *partial_data
            },
            None => Bytes32::default()
        };

        let partial_data_as_array = partial_data.as_u64_array_mut();
        partial_data_as_array[3] = value.try_into().expect("Nonce overflowed");
        
        self.tree.cold_storage.insert(flat_key, partial_data);
        self.tree.storage_tree.insert(
            &flat_key,
            &partial_data,
        );
    }

    pub fn get_storage_slot(
        &mut self,
        address: Address,
        key: web3::types::U256,
    ) -> Option<web3::types::H256> {
        let address = address_to_b160(address);
        let key = h256_to_bytes32(u256_to_h256(key));
        let flat_key = derive_flat_storage_key(&address, &key);

        let value = self.tree.cold_storage.get(&flat_key);
        if let Some(res) = value {
            Some(bytes32_to_h256(*res))
        } else {
            None
        }
    }

    pub fn set_storage_slot(
        &mut self,
        address: Address,
        key: web3::types::U256,
        value: web3::types::H256
    ) {
        let address = address_to_b160(address);
        let key = h256_to_bytes32(u256_to_h256(key));
        let flat_key = derive_flat_storage_key(&address, &key);

        let value = h256_to_bytes32(value);
        self.tree.cold_storage.insert(flat_key, value);
        self.tree.storage_tree.insert(
            &flat_key,
            &value,
        );
    }

    pub fn set_predeployed_evm_contract(
        &mut self,
        address: web3::types::Address,
        bytecode: Vec<u8>,
    ) {
        let address = address_to_b160(address);

        let (mut account_data, mut bytecode_hash) = evm_bytecode_into_partial_account_data(&bytecode);
        PreimageType::Bytecode(ExecutionEnvironmentType::EVM).mark_hash(&mut bytecode_hash);
        self.preimage_source.inner.insert(
            (
                PreimageType::Bytecode(ExecutionEnvironmentType::EVM),
                bytecode_hash,
            ),
            bytecode.to_vec(),
        );
    
        // Now we have to do 2 things:
        // * mark that this account has this bytecode hash deployed
        // * update account state - to say that this is EVM bytecode and nonce is 1.
    
        // We are updating both cold storage (hash map) and our storage tree.
    
        let key = address_into_special_storage_key(&address);
    
        let flat_key = derive_flat_storage_key(&BYTECODE_HASH_STORAGE_ADDRESS, &key);
        self.tree.cold_storage.insert(flat_key, bytecode_hash);
        self.tree.storage_tree.insert(&flat_key, &bytecode_hash);

        account_data.nonce = 1;
        let flat_key = derive_flat_storage_key(&ACCOUNT_PARTIAL_DATA_STORAGE_ADDRESS, &key);
        self.tree.cold_storage
            .insert(flat_key, account_data.pack_to_bytes32());
        self.tree.storage_tree
            .insert(&flat_key, &account_data.pack_to_bytes32());
    }

    pub fn get_code(&mut self, address: Address) -> Option<Vec<u8>> {
        let address = address_to_b160(address);
        let key = address_into_special_storage_key(&address);
        let flat_key = derive_flat_storage_key(&BYTECODE_HASH_STORAGE_ADDRESS, &key);

        let bytecode_hash = self.tree.cold_storage.get(&flat_key);

        match bytecode_hash {
            Some(bytecode_hash) => {
                let preimage = self.preimage_source.get_preimage(PreimageType::Bytecode(ExecutionEnvironmentType::EVM), *bytecode_hash);
                assert!(preimage.is_some(), "Unknown bytecode hash: {bytecode_hash:?}");
                preimage
            },
            None => None,
        }
    }
}

pub fn h256_to_bytes32(input: H256) -> Bytes32 {
    let mut new = Bytes32::zero();
    new.as_u8_array_mut().copy_from_slice(input.as_bytes());
    new
}

pub fn bytes32_to_h256(input: Bytes32) -> H256 {
    H256::from_slice(&input.as_u8_array())
}

pub fn address_to_b160(input: Address) -> B160 {
    B160::from_be_bytes(input.to_fixed_bytes())
}