//!
//! The evm tester library.
//!

#![feature(allocator_api)]
#![allow(non_camel_case_types)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]

pub(crate) mod environment;
pub(crate) mod filters;
pub(crate) mod summary;
pub(crate) mod test;
pub(crate) mod test_suits;
pub(crate) mod utils;
pub(crate) mod vm;
pub(crate) mod workflow;

use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use test::Test;

pub use crate::environment::Environment;
pub use crate::filters::Filters;
pub use crate::summary::Summary;
pub use crate::test_suits::ethereum_general_state::EthereumGeneralStateTestsDirectory;
pub use crate::test_suits::Collection;
pub use crate::vm::eravm::deployers::dummy_deployer::DummyDeployer as EraVMNativeDeployer;
pub use crate::vm::eravm::deployers::system_contract_deployer::SystemContractDeployer as EraVMSystemContractDeployer;
pub use crate::vm::eravm::deployers::EraVMDeployer;
pub use crate::vm::eravm::EraVM;
pub use crate::vm::zk_ee::ZkOS;
pub use crate::workflow::Workflow;

///
/// The evm tester.
///
pub struct EvmTester {
    /// The summary.
    pub summary: Arc<Mutex<Summary>>,
    /// The filters.
    pub filters: Filters,
    /// Actions to perform.
    pub workflow: Workflow,
}

impl EvmTester {
    /// The General state transition tests directory.
    const GENERAL_STATE_TESTS: &'static str = "ethereum-tests/GeneralStateTests";
    const GENERAL_STATE_TESTS_FILLER: &'static str = "ethereum-tests/src/GeneralStateTestsFiller";
}

impl EvmTester {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(
        summary: Arc<Mutex<Summary>>,
        filters: Filters,
        workflow: Workflow,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            summary,
            filters,
            workflow,
        })
    }

    ///
    /// Runs all tests on EVM interpreter.
    ///
    pub fn run_evm_interpreter<D, const M: bool>(self, vm: EraVM) -> anyhow::Result<()>
    where
        D: EraVMDeployer,
    {
        let tests = self.all_tests(Environment::EVMEmulator)?;
        let vm = Arc::new(vm);

        let _: Vec<()> = tests
            .into_par_iter()
            .map(|test| {
                test.run_evm_interpreter::<D, M>(self.summary.clone(), vm.clone());
            })
            .collect();

        Ok(())
    }

    ///
    /// Runs all tests on ZK OS.
    ///
    pub fn run_zk_os(self, vm: ZkOS) -> anyhow::Result<()> {
        let tests = self.all_tests(Environment::ZkOS)?;
        let vm = Arc::new(vm);

        let _: Vec<()> = tests
            .into_par_iter()
            .map(|test| {
                test.run_zk_os(self.summary.clone(), vm.clone());
            })
            .collect();

        Ok(())
    }

    ///
    /// Returns all tests from all directories.
    ///
    fn all_tests(&self, environment: Environment) -> anyhow::Result<Vec<Test>> {
        let mut tests = Vec::with_capacity(16384);

        tests.extend(self.directory::<EthereumGeneralStateTestsDirectory>(
            Self::GENERAL_STATE_TESTS,
            Self::GENERAL_STATE_TESTS_FILLER,
            environment,
        )?);

        Ok(tests)
    }

    ///
    /// Returns all tests from the specified directory.
    ///
    fn directory<T>(
        &self,
        path: &str,
        filler_path: &str,
        environment: Environment,
    ) -> anyhow::Result<Vec<Test>>
    where
        T: Collection,
    {
        T::read_all(
            Path::new(path),
            Path::new(filler_path),
            &self.filters,
            environment,
        )
        .map_err(|error| anyhow::anyhow!("Failed to read the tests directory `{path}`: {error}"))
    }
}
