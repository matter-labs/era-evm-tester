use std::alloc::Global;
use std::cmp::min;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use crate::utils::*;
use alloy::primitives::*;
use itertools::Itertools;
use zk_ee::common_structs::derive_flat_storage_key;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::metadata::BlockHashes;
use zk_ee::system::tracer::NopTracer;
use zk_ee::utils::Bytes32;
use zksync_os_basic_bootloader::bootloader::constants::MAX_BLOCK_GAS_LIMIT;
use zksync_os_basic_bootloader::bootloader::errors::InvalidTransaction;
use zksync_os_basic_system::system_implementation::flat_storage_model::address_into_special_storage_key;
use zksync_os_basic_system::system_implementation::flat_storage_model::AccountProperties;
use zksync_os_basic_system::system_implementation::flat_storage_model::TestingTree;
use zksync_os_basic_system::system_implementation::flat_storage_model::ACCOUNT_PROPERTIES_STORAGE_ADDRESS;
use zksync_os_forward_system::run::errors::ForwardSubsystemError;
use zksync_os_forward_system::run::test_impl::{
    InMemoryPreimageSource, InMemoryTree, NoopTxCallback, TxListSource,
};
use zksync_os_forward_system::run::{
    run_batch_with_oracle_dump, BlockContext, BlockOutput, PreimageSource, StorageCommitment,
    TxOutput,
};
use zksync_os_rig::zksync_os_api::helpers;

use crate::test::case::transaction::AccessListItem;
use crate::test::case::transaction::AuthorizationListItem;
use crate::test::case::transaction::Transaction;

// mod transaction;

#[derive(Clone, Default)]
pub struct ZKsyncOSEVMContext {
    pub chain_id: u64,
    pub coinbase: Address,
    pub block_number: u128,
    pub block_timestamp: u128,
    pub block_gas_limit: U256,
    pub block_difficulty: B256,
    pub base_fee: U256,
    pub gas_price: U256,
    pub tx_origin: Address,
}

///
/// The VM execution result.
///
#[derive(Debug, Clone, Default)]
pub struct ZKsyncOSExecutionResult {
    /// The VM snapshot execution result.
    pub return_data: Vec<u8>,
    pub exception: bool,
    /// The number of gas used.
    pub gas: U256,
    pub address_deployed: Option<Address>,
}

///
/// The ZKsync OS interface.
///
#[derive(Clone)]
pub struct ZKsyncOS {
    tree: InMemoryTree,
    preimage_source: InMemoryPreimageSource,
}

impl ZKsyncOS {
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
        system_context: ZKsyncOSEVMContext,
        bench: bool,
        test_id: String,
    ) -> anyhow::Result<ZKsyncOSExecutionResult, String> {
        let access_list = transaction.access_list.clone().map(|v| {
            alloy::eips::eip2930::AccessList(
                v.into_iter()
                    .map(
                        |AccessListItem {
                             address,
                             storage_keys,
                         }| {
                            let storage_keys = storage_keys
                                .into_iter()
                                .map(|k| {
                                    let buffer: [u8; 32] = k.to_be_bytes();
                                    alloy::primitives::FixedBytes::from_slice(&buffer)
                                })
                                .collect_vec();
                            alloy::eips::eip2930::AccessListItem {
                                address: alloy::primitives::Address::from_slice(address.as_ref()),
                                storage_keys,
                            }
                        },
                    )
                    .collect_vec(),
            )
        });

        #[allow(deprecated)]
        use alloy::primitives::Signature;

        let authorization_list = transaction.authorization_list.clone().map(|v| {
            v.into_iter()
                .map(
                    |AuthorizationListItem {
                         nonce,
                         chain_id,
                         address,
                         v: _,
                         r,
                         s,
                         signer: _,
                         y_parity,
                     }| {
                        let mut r_buf = [0u8; 32];
                        r.to_big_endian(&mut r_buf);
                        let mut s_buf = [0u8; 32];
                        s.to_big_endian(&mut s_buf);
                        let y_parity = !y_parity.is_zero();

                        #[allow(deprecated)]
                        let signature = Signature::from_scalars_and_parity(
                            alloy::primitives::FixedBytes::from_slice(&r_buf),
                            alloy::primitives::FixedBytes::from_slice(&s_buf),
                            y_parity,
                        );
                        alloy::eips::eip7702::Authorization {
                            chain_id: chain_id.into(),
                            nonce: nonce.as_u64(),
                            address: alloy::primitives::Address::from_slice(address.as_ref()),
                        }
                        .into_signed(signature)
                    },
                )
                .collect_vec()
        });

        let request = alloy::rpc::types::TransactionRequest {
            chain_id: Some(system_context.chain_id),
            nonce: Some(transaction.nonce.try_into().expect("Nonce overflow")),
            max_fee_per_gas: Some(
                transaction
                    .max_fee_per_gas
                    .unwrap_or(system_context.gas_price)
                    .try_into()
                    .expect("Max fee per gas overflow"),
            ),
            max_priority_fee_per_gas: Some(
                transaction
                    .max_priority_fee_per_gas
                    .unwrap_or(system_context.gas_price)
                    .try_into()
                    .expect("Max priority fee per gas overflow"),
            ),
            gas: Some(
                transaction
                    .gas_limit
                    .try_into()
                    .expect("gas limit overflow"),
            ),
            to: Some(
                transaction
                    .to
                    .0
                    .map_or(alloy::primitives::TxKind::Create, |addr| {
                        alloy::primitives::TxKind::Call(alloy::primitives::Address::from_slice(
                            addr.as_ref(),
                        ))
                    }),
            ),
            value: Some(transaction.value.into()),
            input: transaction.data.clone().into(),
            access_list,
            authorization_list,
            ..Default::default()
        };

        let wallet = zksync_os_rig::alloy::signers::local::PrivateKeySigner::from_slice(
            transaction.secret_key.as_slice(),
        )
        .unwrap();
        let encoded_tx = helpers::sign_and_encode_transaction_request(request, &wallet);

        let tx_source = TxListSource {
            transactions: vec![encoded_tx].into(),
        };

        let block_gas_limit: u64 = system_context
            .block_gas_limit
            .try_into()
            .expect("Block gas limit overflowed u64");
        // Override block gas limit
        let gas_limit = min(block_gas_limit, MAX_BLOCK_GAS_LIMIT);

        let context = BlockContext {
            //todo: gas
            eip1559_basefee: ruint::Uint::from_str(&system_context.base_fee.to_string())
                .expect("Invalid basefee"),
            native_price: ruint::aliases::U256::from(1),
            gas_per_pubdata: Default::default(),
            block_number: system_context.block_number as u64,
            timestamp: system_context.block_timestamp as u64,
            chain_id: system_context.chain_id,
            gas_limit,
            pubdata_limit: u64::MAX,
            coinbase: ruint::Bits::try_from_be_slice(system_context.coinbase.as_slice())
                .expect("Invalid coinbase"),
            block_hashes: BlockHashes::default(),
            mix_hash: ruint::aliases::U256::from(1),
        };

        let storage_commitment = StorageCommitment {
            root: self.tree.storage_tree.root().clone(),
            next_free_slot: self.tree.storage_tree.next_free_slot,
        };

        let tree = self.tree.clone();
        let preimage_source = self.preimage_source.clone();

        // Output flamegraphs if on benchmarking mode
        if bench {
            use zk_ee::common_structs::ProofData;
            use zk_ee::types_config::EthereumIOTypesConfig;
            use zksync_os_forward_system::run::ForwardRunningOracle;
            use zksync_os_oracle_provider::BasicZkEEOracleWrapper;
            use zksync_os_oracle_provider::ReadWitnessSource;
            use zksync_os_oracle_provider::ZkEENonDeterminismSource;

            let oracle: ForwardRunningOracle<InMemoryTree, InMemoryPreimageSource, TxListSource> =
                ForwardRunningOracle {
                    proof_data: Some(ProofData {
                        state_root_view: storage_commitment,
                        last_block_timestamp: 0,
                    }),
                    block_metadata: context,
                    tree: tree.clone(),
                    preimage_source: preimage_source.clone(),
                    tx_source: tx_source.clone(),
                    next_tx: None,
                };
            let oracle_wrapper =
                BasicZkEEOracleWrapper::<EthereumIOTypesConfig, _>::new(oracle.clone());
            let mut non_determinism_source = ZkEENonDeterminismSource::default();
            non_determinism_source.add_external_processor(oracle_wrapper);
            let copy_source = ReadWitnessSource::new(non_determinism_source);
            let path = std::env::current_dir()
                .unwrap()
                .join(format!("{}.svg", test_id));
            let _output = zksync_os_runner::run_default_with_flamegraph_path(
                1 << 25,
                copy_source,
                Some(path),
            );
        }

        let result = run_batch_with_oracle_dump(
            context,
            tree,
            preimage_source,
            tx_source,
            NoopTxCallback,
            &mut NopTracer::default(),
        );

        self.apply_batch_execution_result(result)
    }

    fn apply_batch_execution_result(
        &mut self,
        batch_execution_result: Result<BlockOutput, ForwardSubsystemError>,
    ) -> anyhow::Result<ZKsyncOSExecutionResult, String> {
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

                for (hash, preimage, _) in result.published_preimages.iter() {
                    self.preimage_source.inner.insert(*hash, preimage.clone());
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
    ) -> anyhow::Result<ZKsyncOSExecutionResult, String> {
        match tx_result {
            Ok(tx_output) => {
                let mut execution_result = ZKsyncOSExecutionResult::default();

                execution_result.gas = U256::from(tx_output.gas_used);
                // TODO events

                match &tx_output.execution_result {
                    zksync_os_forward_system::run::ExecutionResult::Success(execution_output) => {
                        match execution_output {
                            zksync_os_forward_system::run::ExecutionOutput::Call(data) => {
                                execution_result.return_data = data.clone();
                            }
                            zksync_os_forward_system::run::ExecutionOutput::Create(
                                data,
                                address,
                            ) => {
                                let bytes = address.to_be_bytes();
                                execution_result.return_data = data.clone();
                                execution_result.address_deployed = Some(Address::from(bytes));
                            }
                        }
                    }
                    zksync_os_forward_system::run::ExecutionResult::Revert(vec) => {
                        execution_result.exception = true;
                        execution_result.return_data = vec.clone();
                    }
                }
                Ok(execution_result)
            }
            Err(tx_err) => Err(format!("{tx_err:?}")),
        }
    }

    fn get_account_properties(&mut self, address: Address) -> AccountProperties {
        let key =
            address_into_special_storage_key(&ruint::aliases::B160::from_be_bytes(address.0 .0));
        let flat_key = derive_flat_storage_key(&ACCOUNT_PROPERTIES_STORAGE_ADDRESS, &key);
        match self.tree.cold_storage.get(&flat_key) {
            None => AccountProperties::default(),
            Some(account_hash) => {
                if account_hash.is_zero() {
                    // Empty (default) account
                    AccountProperties::default()
                } else {
                    // Get from preimage:
                    let encoded = self
                        .preimage_source
                        .get_preimage(*account_hash)
                        .unwrap_or_default();
                    AccountProperties::decode(&encoded.try_into().unwrap())
                }
            }
        }
    }

    fn set_account_properties(&mut self, address: Address, properties: AccountProperties) {
        let encoding = properties.encoding();
        let properties_hash = properties.compute_hash();
        let address = address_to_b160(address);

        // Save preimage
        self.preimage_source
            .inner
            .insert(properties_hash, encoding.to_vec());

        // Save account hash
        let key = address_into_special_storage_key(&address);
        let flat_key = derive_flat_storage_key(&ACCOUNT_PROPERTIES_STORAGE_ADDRESS, &key);
        self.tree.cold_storage.insert(flat_key, properties_hash);
        self.tree.storage_tree.insert(&flat_key, &properties_hash);
    }

    ///
    /// Returns the balance of the specified address.
    ///
    pub fn get_balance(&mut self, address: Address) -> U256 {
        let properties = self.get_account_properties(address);
        helpers::get_balance(&properties)
    }

    ///
    /// Changes the balance of the specified address.
    ///
    pub fn set_balance(&mut self, address: Address, value: U256) {
        let mut properties = self.get_account_properties(address);
        helpers::set_properties_balance(&mut properties, value);
        self.set_account_properties(address, properties)
    }

    ///
    /// Returns the nonce of the specified address.
    ///
    pub fn get_nonce(&mut self, address: Address) -> U256 {
        let properties = self.get_account_properties(address);
        U256::from(helpers::get_nonce(&properties))
    }

    ///
    /// Changes the nonce of the specified address.
    ///
    pub fn set_nonce(&mut self, address: Address, value: U256) {
        let mut properties = self.get_account_properties(address);
        helpers::set_properties_nonce(&mut properties, value.try_into().expect("nonce overflow"));
        self.set_account_properties(address, properties)
    }

    pub fn get_storage_slot(&mut self, address: Address, key: U256) -> Option<B256> {
        let address = address_to_b160(address);
        let key = u256_to_bytes32(key);
        let flat_key = derive_flat_storage_key(&address, &key);

        let value = self.tree.cold_storage.get(&flat_key);
        if let Some(res) = value {
            Some(bytes32_to_b256(*res))
        } else {
            None
        }
    }

    pub fn set_storage_slot(&mut self, address: Address, key: U256, value: B256) {
        let address = address_to_b160(address);
        let key = u256_to_bytes32(key);
        let flat_key = derive_flat_storage_key(&address, &key);

        let value = b256_to_bytes32(value);
        self.tree.cold_storage.insert(flat_key, value);
        self.tree.storage_tree.insert(&flat_key, &value);
    }

    pub fn evm_bytecode_into_account_properties(
        &mut self,
        address: Address,
        bytecode: &[u8],
    ) -> (AccountProperties, Vec<u8>) {
        let mut result = self.get_account_properties(address);
        let full_bytecode = helpers::set_properties_code(&mut result, bytecode);

        (result, full_bytecode)
    }

    pub fn set_predeployed_evm_contract(&mut self, address: Address, bytecode: Bytes, nonce: U256) {
        let (mut account_data, bytecode) =
            self.evm_bytecode_into_account_properties(address, &bytecode);
        account_data.nonce = nonce.try_into().expect("nonce overflow");

        // Now we have to do 2 things:
        // * mark that this account has this bytecode hash deployed
        // * update account state - to say that this is EVM bytecode and nonce is 0.

        // We are updating both cold storage (hash map) and our storage tree.
        let address = address_to_b160(address);
        let key = address_into_special_storage_key(&address);

        let data_hash = account_data.compute_hash();
        let flat_key = derive_flat_storage_key(&ACCOUNT_PROPERTIES_STORAGE_ADDRESS, &key);
        self.tree.cold_storage.insert(flat_key, data_hash);
        self.tree.storage_tree.insert(&flat_key, &data_hash);
        self.preimage_source
            .inner
            .insert(account_data.bytecode_hash, bytecode.to_vec());
        self.preimage_source
            .inner
            .insert(data_hash, account_data.encoding().to_vec());
    }

    pub fn get_code(&mut self, address: Address) -> Option<Vec<u8>> {
        let properties = self.get_account_properties(address);
        let bytecode_hash = properties.bytecode_hash;

        if bytecode_hash == Bytes32::zero() {
            None
        } else {
            Some(helpers::get_code(&mut self.preimage_source, &properties))
        }
    }
}

pub fn b256_to_bytes32(input: B256) -> Bytes32 {
    Bytes32::from_array(input.0)
}

pub fn u256_to_bytes32(input: U256) -> Bytes32 {
    Bytes32::from_array(input.to_be_bytes())
}

pub fn bytes32_to_b256(input: Bytes32) -> B256 {
    B256::from_slice(&input.as_u8_array())
}
