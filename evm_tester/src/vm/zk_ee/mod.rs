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
use zk_os_basic_system::basic_io_implementer::io_implementer::{
    ACCOUNT_PARTIAL_DATA_STORAGE_ADDRESS, BYTECODE_HASH_STORAGE_ADDRESS,
    NOMINAL_TOKEN_BALANCE_STORAGE_ADDRESS,
};
use zk_os_basic_system::basic_system::simple_growable_storage::TestingTree;
use zk_os_basic_system::basic_system::BlockHashes;
use zk_os_evm_interpreter::utils::evm_bytecode_into_partial_account_data;
use zk_os_forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree, TxListSource};
use zk_os_forward_system::run::{
    run_batch, run_batch_with_oracle_dump, BatchContext, BatchOutput, PreimageSource, PreimageType,
    StorageCommitment, TxOutput,
};
use zksync_types::fee::Fee;
use zksync_types::{K256PrivateKey, H256, U256};

use crate::test::case::transaction::Transaction;

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
    pub address_deployed: Option<Address>,
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
    pub fn new() -> Self {
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

    pub fn clone(vm: Arc<Self>) -> Self {
        (*vm).clone()
    }

    pub fn execute_transaction(
        &mut self,
        transaction: &Transaction,
        system_context: ZkOsEVMContext,
        bench: bool,
        test_id: String,
    ) -> anyhow::Result<ZkOsExecutionResult, String> {
        let tx_type = if transaction.max_priority_fee_per_gas.is_some() {
            Some(2.into())
        } else {
            None
        };
        let fee = Fee {
            gas_limit: transaction.gas_limit,
            max_fee_per_gas: transaction
                .max_fee_per_gas
                .unwrap_or(system_context.gas_price),
            max_priority_fee_per_gas: transaction
                .max_priority_fee_per_gas
                .unwrap_or(system_context.gas_price),
            gas_per_pubdata_limit: Default::default(),
        };

        let l2_tx = gen_l2_tx(
            &K256PrivateKey::from_bytes(transaction.secret_key).expect("Invalid private key"),
            transaction.to.0,
            transaction.data.0.clone(),
            transaction.value,
            transaction.nonce.try_into().expect("Nonce overflow"),
            fee,
            system_context.block_timestamp as u64,
            system_context.chain_id,
            tx_type,
        )
        .context("Gen l2 tx")
        .unwrap();

        let tx = TransactionData::from(l2_tx);

        let encoded_tx = tx.abi_encode();

        let tx_source = TxListSource {
            transactions: vec![encoded_tx].into(),
        };

        let context = BatchContext {
            //todo: gas
            eip1559_basefee: ruint::Uint::from_str(&system_context.base_fee.to_string())
                .expect("Invalid basefee"),
            gas_per_pubdata: Default::default(),
            block_number: system_context.block_number as u64,
            timestamp: system_context.block_timestamp as u64,
            chain_id: system_context.chain_id,
            gas_limit: system_context
                .block_gas_limit
                .try_into()
                .expect("Block gas limit overflowed u64"),
            coinbase: ruint::Bits::try_from_be_slice(system_context.coinbase.as_bytes())
                .expect("Invalid coinbase"),
            block_hashes: BlockHashes::default(),
        };

        let storage_commitment = StorageCommitment {
            root: self.tree.storage_tree.root().clone(),
            next_free_slot: self.tree.storage_tree.next_free_slot,
        };

        let tree = self.tree.clone();
        let preimage_source = self.preimage_source.clone();

        // Output flamegraphs if on benchmarking mode
        if bench {
            use zk_os_forward_system::run::io_implementer_init_data;
            use zk_os_forward_system::run::ForwardRunningOracle;
            use zk_os_oracle_provider::BasicZkEEOracleWrapper;
            use zk_os_oracle_provider::ReadWitnessSource;
            use zk_os_oracle_provider::ZkEENonDeterminismSource;

            let oracle: ForwardRunningOracle<InMemoryTree, InMemoryPreimageSource, TxListSource> =
                ForwardRunningOracle {
                    io_implementer_init_data: Some(io_implementer_init_data(Some(
                        storage_commitment,
                    ))),
                    block_metadata: context,
                    tree: tree.clone(),
                    preimage_source: preimage_source.clone(),
                    tx_source: tx_source.clone(),
                    next_tx: None,
                };
            let oracle_wrapper = BasicZkEEOracleWrapper::new(oracle.clone());
            let mut non_determinism_source = ZkEENonDeterminismSource::default();
            non_determinism_source.add_external_processor(oracle_wrapper);
            let copy_source = ReadWitnessSource::new(non_determinism_source);
            let path = std::env::current_dir()
                .unwrap()
                .join(format!("{}.svg", test_id));
            let _output =
                zk_os_runner::run_default_with_flamegraph_path(1 << 25, copy_source, Some(path));
        }

        let result = run_batch_with_oracle_dump(
            context,
            storage_commitment,
            tree,
            preimage_source,
            tx_source,
        );

        self.apply_batch_execution_result(result)
    }

    fn apply_batch_execution_result(
        &mut self,
        batch_execution_result: Result<BatchOutput, InternalError>,
    ) -> anyhow::Result<ZkOsExecutionResult, String> {
        match batch_execution_result {
            Ok(result) => {
                for storage_write in result.storage_writes.iter() {
                    self.tree
                        .cold_storage
                        .insert(storage_write.key, storage_write.value);
                    self.tree
                        .storage_tree
                        .insert(&storage_write.key, &storage_write.value);
                }

                for (hash, preimage) in result.published_preimages.iter() {
                    self.preimage_source.inner.insert(
                        (PreimageType::Bytecode(ExecutionEnvironmentType::EVM), *hash),
                        preimage.clone(),
                    );
                }

                let tx_result = result
                    .tx_results
                    .get(0)
                    .expect("Do not have tx output")
                    .clone();

                Self::get_transaction_execution_result(tx_result)
            }
            Err(err) => Err(format!("{err:?}")),
        }
    }

    fn get_transaction_execution_result(
        tx_result: Result<TxOutput, InvalidTransaction>,
    ) -> anyhow::Result<ZkOsExecutionResult, String> {
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
                            }
                            zk_os_forward_system::run::ExecutionOutput::Create(data, address) => {
                                let bytes = address.to_be_bytes();
                                execution_result.return_data = data.clone();
                                execution_result.address_deployed = Some(Address::from(bytes));
                            }
                        }
                    }
                    zk_os_forward_system::run::ExecutionResult::Revert(vec) => {
                        execution_result.exception = true;
                        execution_result.return_data = vec.clone();
                    }
                }
                Ok(execution_result)
            }
            Err(tx_err) => Err(format!("{tx_err:?}")),
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
        self.tree.cold_storage.insert(flat_key, value);
        self.tree.storage_tree.insert(&flat_key, &value);
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
                let nonce = zk_ee::system::reference_implementations::storage_format::account_code::PackedPartialAccountData::read_nonce_from_bytes32_encoding(partial_data);
                nonce.into()
            }
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
        use zk_ee::system::reference_implementations::storage_format::account_code::*;

        let mut partial_data: PackedPartialAccountData = match self.tree.cold_storage.get(&flat_key)
        {
            Some(partial_data) => PackedPartialAccountData::from_bytes32_encoding(partial_data),
            None => PackedPartialAccountData::empty(),
        };

        partial_data.nonce = value.try_into().expect("nonce overflow");
        let packed = partial_data.pack_to_bytes32();

        self.tree.cold_storage.insert(flat_key, packed);
        self.tree.storage_tree.insert(&flat_key, &packed);
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
        value: web3::types::H256,
    ) {
        let address = address_to_b160(address);
        let key = h256_to_bytes32(u256_to_h256(key));
        let flat_key = derive_flat_storage_key(&address, &key);

        let value = h256_to_bytes32(value);
        self.tree.cold_storage.insert(flat_key, value);
        self.tree.storage_tree.insert(&flat_key, &value);
    }

    pub fn set_predeployed_evm_contract(
        &mut self,
        address: web3::types::Address,
        bytecode: Vec<u8>,
        nonce: U256,
    ) {
        let address = address_to_b160(address);

        let (mut account_data, bytecode_hash) = evm_bytecode_into_partial_account_data(&bytecode);
        self.preimage_source.inner.insert(
            (
                PreimageType::Bytecode(ExecutionEnvironmentType::EVM),
                bytecode_hash,
            ),
            bytecode.to_vec(),
        );
        account_data.nonce = nonce.try_into().expect("nonce overflow");

        // Now we have to do 2 things:
        // * mark that this account has this bytecode hash deployed
        // * update account state - to say that this is EVM bytecode.

        // We are updating both cold storage (hash map) and our storage tree.

        let key = address_into_special_storage_key(&address);

        let flat_key = derive_flat_storage_key(&BYTECODE_HASH_STORAGE_ADDRESS, &key);
        self.tree.cold_storage.insert(flat_key, bytecode_hash);
        self.tree.storage_tree.insert(&flat_key, &bytecode_hash);

        let flat_key = derive_flat_storage_key(&ACCOUNT_PARTIAL_DATA_STORAGE_ADDRESS, &key);
        self.tree
            .cold_storage
            .insert(flat_key, account_data.pack_to_bytes32());
        self.tree
            .storage_tree
            .insert(&flat_key, &account_data.pack_to_bytes32());
    }

    pub fn get_code(&mut self, address: Address) -> Option<Vec<u8>> {
        let address = address_to_b160(address);
        let key = address_into_special_storage_key(&address);
        let flat_key = derive_flat_storage_key(&BYTECODE_HASH_STORAGE_ADDRESS, &key);

        let bytecode_hash = self.tree.cold_storage.get(&flat_key);

        match bytecode_hash {
            Some(bytecode_hash) => {
                if *bytecode_hash == Bytes32::zero() {
                    None
                } else {
                    let preimage = self.preimage_source.get_preimage(
                        PreimageType::Bytecode(ExecutionEnvironmentType::EVM),
                        *bytecode_hash,
                    );
                    assert!(
                        preimage.is_some(),
                        "Unknown bytecode hash: {bytecode_hash:?}"
                    );
                    preimage
                }
            }
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
