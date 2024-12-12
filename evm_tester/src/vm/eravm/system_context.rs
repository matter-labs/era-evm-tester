//!
//! The EraVM system context.
//!

use std::collections::HashMap;
use std::ops::Add;
use std::str::FromStr;

use super::utils;

///
/// The EraVM system context.
///
pub struct SystemContext;

#[derive(Clone)]
pub struct EVMContext {
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

impl SystemContext {
    /// The system context chain ID value position in the storage.
    pub const SYSTEM_CONTEXT_CHAIN_ID_POSITION: u64 = 0;

    /// The system context origin value position in the storage.
    pub const SYSTEM_CONTEXT_ORIGIN_POSITION: u64 = 1;

    /// The system context gas price value position in the storage.
    pub const SYSTEM_CONTEXT_GAS_PRICE_POSITION: u64 = 2;

    /// The system context block gas limit value position in the storage.
    pub const SYSTEM_CONTEXT_BLOCK_GAS_LIMIT_POSITION: u64 = 3;

    /// The system context coinbase value position in the storage.
    pub const SYSTEM_CONTEXT_COINBASE_POSITION: u64 = 4;

    /// The system context difficulty value position in the storage.
    pub const SYSTEM_CONTEXT_DIFFICULTY_POSITION: u64 = 5;

    /// The system context base fee value position in the storage.
    pub const SYSTEM_CONTEXT_BASE_FEE_POSITION: u64 = 6;

    /// The system context block hashes mapping position in the storage.
    pub const SYSTEM_CONTEXT_BLOCK_HASH_POSITION: u64 = 8;

    /// The system context current virtual L2 block info value position in the storage.
    pub const SYSTEM_CONTEXT_VIRTUAL_L2_BLOCK_INFO_POSITION: u64 = 268;

    /// The system context virtual blocks upgrade info position in the storage.
    pub const SYSTEM_CONTEXT_VIRTUAL_BLOCK_UPGRADE_INFO_POSITION: u64 = 269;

    /// The ZKsync chain ID.
    pub const CHAIND_ID_ERAVM: u64 = 280;
    /// The Ethereum chain ID.
    pub const CHAIND_ID_EVM: u64 = 1;

    /// The default origin for tests.
    pub const TX_ORIGIN: &'static str =
        "0x0000000000000000000000009292929292929292929292929292929292929292";

    /// The default gas price for tests.
    pub const GAS_PRICE: u64 = 3000000000;

    /// The default block gas limit for EraVM tests.
    pub const BLOCK_GAS_LIMIT_ERAVM: u64 = (1 << 30);
    /// The default block gas limit for EVM tests.
    pub const BLOCK_GAS_LIMIT_EVM: u64 = 20000000;

    /// The default coinbase for EraVM tests.
    pub const COIN_BASE_ERAVM: &'static str =
        "0x0000000000000000000000000000000000000000000000000000000000008001";
    /// The default coinbase for EVM tests.
    pub const COIN_BASE_EVM: &'static str =
        "0x0000000000000000000000007878787878787878787878787878787878787878";

    /// The default block difficulty for EraVM tests.
    pub const BLOCK_DIFFICULTY_ERAVM: u64 = 2500000000000000;
    /// The block difficulty for EVM tests using a post paris version.
    pub const BLOCK_DIFFICULTY_EVM_POST_PARIS: &'static str =
        "0xa86c2e601b6c44eb4848f7d23d9df3113fbcac42041c49cbed5000cb4f118777";
    /// The block difficulty for EVM tests using a pre paris version.
    pub const BLOCK_DIFFICULTY_EVM_PRE_PARIS: &'static str =
        "0x000000000000000000000000000000000000000000000000000000000bebc200";

    /// The default base fee for tests.
    pub const BASE_FEE: u64 = 7;

    /// The default current block number.
    pub const INITIAL_BLOCK_NUMBER: u128 = 1;
    /// The default current block number.
    pub const CURRENT_BLOCK_NUMBER: u128 = 2;

    /// The default current block timestamp for EraVM tests.
    pub const CURRENT_BLOCK_TIMESTAMP_ERAVM: u128 = 0xdeadbeef;
    /// The default current block timestamp for EVM tests.
    pub const CURRENT_BLOCK_TIMESTAMP_EVM: u128 = 30;
    /// The timestamp step for blocks in the EVM context.
    pub const BLOCK_TIMESTAMP_EVM_STEP: u128 = 15;

    /// The default zero block hash.
    pub const ZERO_BLOCK_HASH: &'static str =
        "0x3737373737373737373737373737373737373737373737373737373737373737";

    ///
    /// Returns the storage values for the system context.
    ///
    pub fn create_storage(
        _target: era_compiler_common::Target,
    ) -> HashMap<zkevm_tester::compiler_tests::StorageKey, web3::types::H256> {
        HashMap::new()
    }

    pub fn default_context(target: era_compiler_common::Target) -> EVMContext {
        let chain_id = match target {
            era_compiler_common::Target::EraVM => Self::CHAIND_ID_ERAVM,
            era_compiler_common::Target::EVM => Self::CHAIND_ID_EVM,
        };
        let coinbase = match target {
            era_compiler_common::Target::EraVM => Self::COIN_BASE_ERAVM,
            era_compiler_common::Target::EVM => Self::COIN_BASE_EVM,
        };

        let block_number = Self::CURRENT_BLOCK_NUMBER;
        let block_timestamp = match target {
            era_compiler_common::Target::EraVM => Self::CURRENT_BLOCK_TIMESTAMP_ERAVM,
            era_compiler_common::Target::EVM => Self::BLOCK_TIMESTAMP_EVM_STEP,
        };
        let block_gas_limit = match target {
            era_compiler_common::Target::EraVM => Self::BLOCK_GAS_LIMIT_ERAVM,
            era_compiler_common::Target::EVM => Self::BLOCK_GAS_LIMIT_EVM,
        };

        let block_difficulty = match target {
            era_compiler_common::Target::EraVM => {
                web3::types::H256::from_low_u64_be(Self::BLOCK_DIFFICULTY_ERAVM)
            }
            // This block difficulty is set by default, but it can be overridden if the test needs it.
            era_compiler_common::Target::EVM => {
                web3::types::H256::from_str(Self::BLOCK_DIFFICULTY_EVM_POST_PARIS)
                    .expect("Always valid")
            }
        };

        EVMContext {
            chain_id,
            coinbase: web3::types::H256::from_str(coinbase)
                .expect("Always valid")
                .into(),
            block_number,
            block_timestamp,
            block_gas_limit: web3::types::U256::from(block_gas_limit),
            block_difficulty,
            base_fee: web3::types::U256::from(Self::BASE_FEE),
            gas_price: web3::types::U256::from(Self::GAS_PRICE),
            tx_origin: web3::types::H256::from_str(Self::TX_ORIGIN)
                .expect("Always valid")
                .into(),
        }
    }

    pub fn set_system_context(
        storage: &mut HashMap<zkevm_tester::compiler_tests::StorageKey, web3::types::H256>,
        context: &EVMContext,
    ) {
        let mut system_context_values = vec![
            (
                web3::types::H256::from_low_u64_be(Self::SYSTEM_CONTEXT_CHAIN_ID_POSITION),
                web3::types::H256::from_low_u64_be(context.chain_id),
            ),
            (
                web3::types::H256::from_low_u64_be(Self::SYSTEM_CONTEXT_ORIGIN_POSITION),
                context.tx_origin.into(),
            ),
            (
                web3::types::H256::from_low_u64_be(Self::SYSTEM_CONTEXT_GAS_PRICE_POSITION),
                utils::u256_to_h256(&context.gas_price),
            ),
            (
                web3::types::H256::from_low_u64_be(Self::SYSTEM_CONTEXT_BLOCK_GAS_LIMIT_POSITION),
                utils::u256_to_h256(&context.block_gas_limit),
            ),
            (
                web3::types::H256::from_low_u64_be(Self::SYSTEM_CONTEXT_COINBASE_POSITION),
                context.coinbase.into(),
            ),
            (
                web3::types::H256::from_low_u64_be(Self::SYSTEM_CONTEXT_DIFFICULTY_POSITION),
                context.block_difficulty,
            ),
            (
                web3::types::H256::from_low_u64_be(Self::SYSTEM_CONTEXT_BASE_FEE_POSITION),
                utils::u256_to_h256(&context.base_fee),
            ),
            (
                web3::types::H256::from_low_u64_be(
                    Self::SYSTEM_CONTEXT_VIRTUAL_BLOCK_UPGRADE_INFO_POSITION,
                ),
                web3::types::H256::from_low_u64_be(context.block_number as u64),
            ),
            (
                web3::types::H256::from_low_u64_be(
                    Self::SYSTEM_CONTEXT_VIRTUAL_BLOCK_UPGRADE_INFO_POSITION,
                ),
                web3::types::H256::from_low_u64_be(context.block_number as u64),
            ),
        ];

        let block_info_bytes = [
            context.block_number.to_be_bytes(),
            context.block_timestamp.to_be_bytes(),
        ]
        .concat();

        system_context_values.push((
            web3::types::H256::from_low_u64_be(Self::SYSTEM_CONTEXT_VIRTUAL_L2_BLOCK_INFO_POSITION),
            web3::types::H256::from_slice(block_info_bytes.as_slice()),
        ));

        for index in 0..context.block_number {
            let padded_index = [[0u8; 16], index.to_be_bytes()].concat();
            let padded_slot =
                web3::types::H256::from_low_u64_be(Self::SYSTEM_CONTEXT_BLOCK_HASH_POSITION)
                    .to_fixed_bytes()
                    .to_vec();
            let key = web3::signing::keccak256([padded_index, padded_slot].concat().as_slice());

            let mut hash =
                web3::types::U256::from_str(Self::ZERO_BLOCK_HASH).expect("Always valid");
            hash = hash.add(web3::types::U256::from(index));
            let mut hash_bytes = [0u8; era_compiler_common::BYTE_LENGTH_FIELD];
            hash.to_big_endian(&mut hash_bytes);

            system_context_values.push((
                web3::types::H256::from(key),
                web3::types::H256::from_slice(hash_bytes.as_slice()),
            ));
        }

        for (key, value) in system_context_values {
            storage.insert(
                zkevm_tester::compiler_tests::StorageKey {
                    address: web3::types::Address::from_low_u64_be(
                        zkevm_opcode_defs::ADDRESS_SYSTEM_CONTEXT.into(),
                    ),
                    key: web3::types::U256::from_big_endian(key.as_bytes()),
                },
                value,
            );
        }
    }

    ///
    /// Returns addresses that must be funded for testing.
    ///
    pub fn get_rich_addresses() -> Vec<web3::types::Address> {
        (0..=9)
            .map(|address_id| {
                format!(
                    "0x121212121212121212121212121212000000{}{}",
                    address_id, "012"
                )
            })
            .map(|string| web3::types::Address::from_str(&string).unwrap())
            .collect()
    }

    ///
    /// Sets the storage values for the system context to the pre-Paris values.
    ///
    pub fn set_pre_paris_contracts(
        storage: &mut HashMap<zkevm_tester::compiler_tests::StorageKey, web3::types::H256>,
    ) {
        storage.insert(
            zkevm_tester::compiler_tests::StorageKey {
                address: web3::types::Address::from_low_u64_be(
                    zkevm_opcode_defs::ADDRESS_SYSTEM_CONTEXT.into(),
                ),
                key: web3::types::U256::from_big_endian(
                    web3::types::H256::from_low_u64_be(
                        SystemContext::SYSTEM_CONTEXT_DIFFICULTY_POSITION,
                    )
                    .as_bytes(),
                ),
            },
            web3::types::H256::from_str(SystemContext::BLOCK_DIFFICULTY_EVM_PRE_PARIS)
                .expect("Always valid"),
        );
    }
}
