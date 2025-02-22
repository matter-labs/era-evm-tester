//!
//! The EraVM system contracts.
//!

use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Instant;

use zksync_contracts::ContractLanguage;
use zksync_types::bytecode::BytecodeHash;
use zksync_types::{
    block::DeployedContract, AccountTreeId, Address, ACCOUNT_CODE_STORAGE_ADDRESS,
    BOOTLOADER_ADDRESS, BOOTLOADER_UTILITIES_ADDRESS, CODE_ORACLE_ADDRESS,
    COMPLEX_UPGRADER_ADDRESS, COMPRESSOR_ADDRESS, CONTRACT_DEPLOYER_ADDRESS,
    ECRECOVER_PRECOMPILE_ADDRESS, EC_ADD_PRECOMPILE_ADDRESS, EC_MUL_PRECOMPILE_ADDRESS,
    EC_PAIRING_PRECOMPILE_ADDRESS, EVENT_WRITER_ADDRESS, EVM_GAS_MANAGER_ADDRESS, IDENTITY_ADDRESS,
    IMMUTABLE_SIMULATOR_STORAGE_ADDRESS, KECCAK256_PRECOMPILE_ADDRESS, KNOWN_CODES_STORAGE_ADDRESS,
    L1_MESSENGER_ADDRESS, L2_BASE_TOKEN_ADDRESS, MSG_VALUE_SIMULATOR_ADDRESS, NONCE_HOLDER_ADDRESS,
    P256VERIFY_PRECOMPILE_ADDRESS, PUBDATA_CHUNK_PUBLISHER_ADDRESS, SHA256_PRECOMPILE_ADDRESS,
    SYSTEM_CONTEXT_ADDRESS,
};

use colored::Colorize;

/// The EVMGasManager system contract address.
pub const ADDRESS_EVM_GAS_MANAGER: u16 = 0x8013;

pub const ADDRESS_EVM_HASHES_STORAGE: Address = web3::types::H160([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x80, 0x15,
]);

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Build {
    /// The bytecode.
    pub bytecode: Vec<u8>,
    /// The bytecode hash. Only available after linking.
    pub bytecode_hash: Option<[u8; era_compiler_common::BYTE_LENGTH_FIELD]>,
}

pub static SYSTEM_CONTRACT_LIST: [(&str, &str, Address, ContractLanguage); 27] = [
    (
        "",
        "AccountCodeStorage",
        ACCOUNT_CODE_STORAGE_ADDRESS,
        ContractLanguage::Sol,
    ),
    (
        "",
        "NonceHolder",
        NONCE_HOLDER_ADDRESS,
        ContractLanguage::Sol,
    ),
    (
        "",
        "KnownCodesStorage",
        KNOWN_CODES_STORAGE_ADDRESS,
        ContractLanguage::Sol,
    ),
    (
        "",
        "ImmutableSimulator",
        IMMUTABLE_SIMULATOR_STORAGE_ADDRESS,
        ContractLanguage::Sol,
    ),
    (
        "",
        "ContractDeployer",
        CONTRACT_DEPLOYER_ADDRESS,
        ContractLanguage::Sol,
    ),
    (
        "",
        "L1Messenger",
        L1_MESSENGER_ADDRESS,
        ContractLanguage::Sol,
    ),
    (
        "",
        "MsgValueSimulator",
        MSG_VALUE_SIMULATOR_ADDRESS,
        ContractLanguage::Sol,
    ),
    (
        "",
        "L2BaseToken",
        L2_BASE_TOKEN_ADDRESS,
        ContractLanguage::Sol,
    ),
    (
        "precompiles/",
        "Keccak256",
        KECCAK256_PRECOMPILE_ADDRESS,
        ContractLanguage::Yul,
    ),
    (
        "precompiles/",
        "SHA256",
        SHA256_PRECOMPILE_ADDRESS,
        ContractLanguage::Yul,
    ),
    (
        "precompiles/",
        "Ecrecover",
        ECRECOVER_PRECOMPILE_ADDRESS,
        ContractLanguage::Yul,
    ),
    (
        "precompiles/",
        "EcAdd",
        EC_ADD_PRECOMPILE_ADDRESS,
        ContractLanguage::Yul,
    ),
    (
        "precompiles/",
        "EcMul",
        EC_MUL_PRECOMPILE_ADDRESS,
        ContractLanguage::Yul,
    ),
    (
        "precompiles/",
        "EcPairing",
        EC_PAIRING_PRECOMPILE_ADDRESS,
        ContractLanguage::Yul,
    ),
    (
        "precompiles/",
        "P256Verify",
        P256VERIFY_PRECOMPILE_ADDRESS,
        ContractLanguage::Yul,
    ),
    (
        "precompiles/",
        "CodeOracle",
        CODE_ORACLE_ADDRESS,
        ContractLanguage::Yul,
    ),
    (
        "precompiles/",
        "Identity",
        IDENTITY_ADDRESS,
        ContractLanguage::Yul,
    ),
    (
        "",
        "SystemContext",
        SYSTEM_CONTEXT_ADDRESS,
        ContractLanguage::Sol,
    ),
    (
        "",
        "EventWriter",
        EVENT_WRITER_ADDRESS,
        ContractLanguage::Yul,
    ),
    (
        "",
        "BootloaderUtilities",
        BOOTLOADER_UTILITIES_ADDRESS,
        ContractLanguage::Sol,
    ),
    ("", "Compressor", COMPRESSOR_ADDRESS, ContractLanguage::Sol),
    (
        "",
        "ComplexUpgrader",
        COMPLEX_UPGRADER_ADDRESS,
        ContractLanguage::Sol,
    ),
    (
        "",
        "EvmGasManager",
        EVM_GAS_MANAGER_ADDRESS,
        ContractLanguage::Yul,
    ), 
    (
        "",
        "EvmHashesStorage",
        ADDRESS_EVM_HASHES_STORAGE,
        ContractLanguage::Sol,
    ),
    // For now, only zero address and the bootloader address have empty bytecode at the init
    // In the future, we might want to set all of the system contracts this way.
    ("", "EmptyContract", Address::zero(), ContractLanguage::Sol),
    (
        "",
        "EmptyContract",
        BOOTLOADER_ADDRESS,
        ContractLanguage::Sol,
    ),
    (
        "",
        "PubdataChunkPublisher",
        PUBDATA_CHUNK_PUBLISHER_ADDRESS,
        ContractLanguage::Sol,
    ),
];

///
/// The EraVM system contracts.
///
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SystemContracts {
    /// The deployed system contracts builds.
    pub deployed_contracts: Vec<(web3::types::Address, Build)>,
    /// The default account abstraction contract build.
    pub default_aa: Build,
    /// The EVM emulator contract build.
    pub evm_emulator: Build,
}

impl SystemContracts {
    ///
    /// Builds the system contracts.
    ///
    pub fn build() -> anyhow::Result<Self> {
        let build_time_start = Instant::now();
        println!("    {} system contracts", "Building".bright_green().bold());

        let system_contracts_path = PathBuf::from("era-contracts/system-contracts");

        let system_contracts =
            get_system_smart_contracts_from_dir(system_contracts_path.clone(), true);

        println!(
            "    {} building system contracts in {}.{:03}s",
            "Finished".bright_green().bold(),
            build_time_start.elapsed().as_secs(),
            build_time_start.elapsed().subsec_millis(),
        );

        let deployed_contracts: Vec<_> = system_contracts
            .into_iter()
            .map(|contract| (*contract.account_id.address(), contract.bytecode))
            .collect();

        let evm_emulator_bytecode = read_sys_contract_bytecode(
            system_contracts_path.clone(),
            "",
            "EvmEmulator",
            ContractLanguage::Yul,
        );
        let evm_emulator = Build {
            bytecode: evm_emulator_bytecode.clone(),
            bytecode_hash: Some(
                BytecodeHash::for_bytecode(&evm_emulator_bytecode)
                    .value()
                    .to_fixed_bytes(),
            ),
        };

        let default_aa_bytecode = read_sys_contract_bytecode(
            system_contracts_path.clone(),
            "",
            "DefaultAccount",
            ContractLanguage::Sol,
        );
        let default_aa = Build {
            bytecode: default_aa_bytecode.clone(),
            bytecode_hash: Some(
                BytecodeHash::for_bytecode(&default_aa_bytecode)
                    .value()
                    .to_fixed_bytes(),
            ),
        };

        let deployed_contracts = deployed_contracts
            .into_iter()
            .map(|(address, bytecode)| {
                let build = Build {
                    bytecode: bytecode.clone(),
                    bytecode_hash: Some(
                        BytecodeHash::for_bytecode(&bytecode)
                            .value()
                            .to_fixed_bytes(),
                    ),
                };

                (address, build)
            })
            .collect();

        Ok(Self {
            deployed_contracts,
            default_aa,
            evm_emulator,
        })
    }
}

pub fn get_system_smart_contracts_from_dir(
    root: PathBuf,
    use_evm_emulator: bool,
) -> Vec<DeployedContract> {
    SYSTEM_CONTRACT_LIST
        .iter()
        .filter_map(|(path, name, address, contract_lang)| {
            if *name == "EvmGasManager" && !use_evm_emulator {
                None
            } else {
                Some(DeployedContract {
                    account_id: AccountTreeId::new(*address),
                    bytecode: read_sys_contract_bytecode(
                        root.clone(),
                        path,
                        name,
                        contract_lang.clone(),
                    ),
                })
            }
        })
        .collect::<Vec<_>>()
}

pub fn read_sys_contract_bytecode(
    root: PathBuf,
    directory: &str,
    name: &str,
    lang: ContractLanguage,
) -> Vec<u8> {
    match lang {
        ContractLanguage::Sol => {
            if let Some(contracts) = read_bytecode_from_path(
                root.join(format!("zkout/{0}{1}.sol/{1}.json", directory, name)),
            ) {
                contracts
            } else {
                read_bytecode_from_path(root.join(format!(
                    "artifacts-zk/contracts-preprocessed/{0}{1}.sol/{1}.json",
                    directory, name
                )))
                .unwrap_or_else(|| panic!("One of the outputs should exists: {:?} {}", root, name))
            }
        }
        ContractLanguage::Yul => {
            if let Some(contract) = read_bytecode_from_path(root.join(format!(
                "zkout/{name}.yul/contracts-preprocessed/{directory}/{name}.yul.json",
            ))) {
                contract
            } else {
                read_yul_bytecode_by_path(
                    root.join(format!("contracts-preprocessed/{directory}artifacts")),
                    name,
                )
            }
        }
    }
}

/// Reads bytecode from a given path.
pub fn read_bytecode_from_path(
    artifact_path: impl AsRef<Path> + std::fmt::Debug,
) -> Option<Vec<u8>> {
    let artifact = read_file_to_json_value(&artifact_path)?;

    let bytecode = if let Some(bytecode) = artifact["bytecode"].as_str() {
        bytecode
            .strip_prefix("0x")
            .unwrap_or_else(|| panic!("Bytecode in {:?} is not hex", artifact_path))
    } else {
        artifact["bytecode"]["object"]
            .as_str()
            .unwrap_or_else(|| panic!("Bytecode not found in {:?}", artifact_path))
    };

    Some(
        hex::decode(bytecode)
            .unwrap_or_else(|err| panic!("Can't decode bytecode in {:?}: {}", artifact_path, err)),
    )
}

fn read_file_to_json_value(path: impl AsRef<Path> + std::fmt::Debug) -> Option<serde_json::Value> {
    let root = PathBuf::from("");

    let path = root.join(path);

    let file = File::open(&path).ok()?;
    Some(
        serde_json::from_reader(BufReader::new(file))
            .unwrap_or_else(|e| panic!("Failed to parse file {:?}: {}", path, e)),
    )
}

pub fn read_yul_bytecode_by_path(artifacts_path: PathBuf, name: &str) -> Vec<u8> {
    let bytecode_path = artifacts_path.join(format!("{name}.yul/{name}.yul.zbin"));

    // Legacy versions of zksolc use the following path for output data if a yul file is being compiled: <name>.yul.zbin
    // New zksolc versions use <name>.yul/<name>.yul.zbin, for consistency with solidity files compilation.
    // In addition, the output of the legacy zksolc in this case is a binary file, while in new versions it is hex encoded.
    if fs::exists(&bytecode_path)
        .unwrap_or_else(|err| panic!("Invalid path: {bytecode_path:?}, {err}"))
    {
        read_zbin_bytecode_from_hex_file(bytecode_path)
    } else {
        let bytecode_path_legacy = artifacts_path.join(format!("{name}.yul.zbin"));

        if fs::exists(&bytecode_path_legacy)
            .unwrap_or_else(|err| panic!("Invalid path: {bytecode_path_legacy:?}, {err}"))
        {
            read_zbin_bytecode_from_path(bytecode_path_legacy)
        } else {
            panic!("Can't find bytecode for '{name}' yul contract at {artifacts_path:?}")
        }
    }
}

/// Reads zbin bytecode from a given path.
fn read_zbin_bytecode_from_path(bytecode_path: PathBuf) -> Vec<u8> {
    fs::read(&bytecode_path)
        .unwrap_or_else(|err| panic!("Can't read .zbin bytecode at {bytecode_path:?}: {err}"))
}

/// Reads zbin bytecode from a given path as utf8 text file.
fn read_zbin_bytecode_from_hex_file(bytecode_path: PathBuf) -> Vec<u8> {
    let bytes = fs::read(&bytecode_path)
        .unwrap_or_else(|err| panic!("Can't read .zbin bytecode at {bytecode_path:?}: {err}"));

    hex::decode(bytes).unwrap_or_else(|err| panic!("Invalid input file: {bytecode_path:?}, {err}"))
}
