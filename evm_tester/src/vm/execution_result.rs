//!
//! The VM execution result.
//!

use super::output::ExecutionOutput;

///
/// The VM execution result.
///
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// The VM snapshot execution result.
    pub output: ExecutionOutput,
    /// The number of executed cycles.
    pub cycles: usize,
    /// The number of EraVM ergs used.
    pub ergs: u64,
    /// The number of gas used.
    pub gas: web3::types::U256,
}

impl ExecutionResult {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(output: ExecutionOutput, cycles: usize, ergs: u64, gas: web3::types::U256) -> Self {
        Self {
            output,
            cycles,
            ergs,
            gas,
        }
    }
}

impl From<zkevm_tester::compiler_tests::VmSnapshot> for ExecutionResult {
    fn from(snapshot: zkevm_tester::compiler_tests::VmSnapshot) -> Self {
        let cycles = snapshot.num_cycles_used;
        let ergs = snapshot.num_ergs_used as u64;

        Self {
            output: ExecutionOutput::from(snapshot),
            cycles,
            ergs,
            gas: web3::types::U256::zero(),
        }
    }
}
