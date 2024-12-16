use zkevm_opcode_defs::sha2::{Digest, Sha256};
use zksync_types::{bytecode::BytecodeMarker, H256};

/// Hashes the provided padded EVM bytecode.
pub fn hash_evm_bytecode(bytecode_len: u16, bytecode: &[u8]) -> H256 {
    let mut hasher = Sha256::new();
    hasher.update(bytecode);
    let result = hasher.finalize();

    let mut output = [0u8; 32];
    output[..].copy_from_slice(result.as_slice());
    output[0] = BytecodeMarker::Evm as u8;
    output[1] = 0;
    output[2..4].copy_from_slice(&bytecode_len.to_be_bytes());

    H256(output)
}