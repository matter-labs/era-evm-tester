//!
//! The evm tester utils.
//!

use sha3::Digest;

///
/// Returns a `keccak256` selector of the specified contract method.
///
pub fn selector(signature: &str) -> [u8; 4] {
    let hash_bytes = sha3::Keccak256::digest(signature.as_bytes());
    hash_bytes[0..4].try_into().expect("Always valid")
}

pub(crate) fn address_to_b160(value: alloy::primitives::Address) -> ruint::aliases::B160 {
    ruint::aliases::B160::from_be_bytes(value.0 .0)
}
