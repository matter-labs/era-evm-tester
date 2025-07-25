//!
//! The address iterator trait.
//!

use alloy::primitives::*;

///
/// The address iterator trait.
///
pub trait AddressIterator {
    ///
    /// Returns the next address.
    ///
    fn next(&mut self, caller: &Address, increment_nonce: bool) -> Address;

    ///
    /// Increments the nonce for the caller.
    ///
    fn increment_nonce(&mut self, caller: &Address);

    ///
    /// Returns the nonce for the caller.
    ///
    /// If the nonce for the `caller` does not exist, it will be created.
    ///
    fn nonce(&mut self, caller: &Address) -> usize;
}
