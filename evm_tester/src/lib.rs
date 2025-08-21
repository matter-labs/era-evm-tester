//!
//! The evm tester library.
//!

#![feature(allocator_api)]
#![allow(non_camel_case_types)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

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
use test_suits::ethereum_execution_specs_general_state::EthereumExecutionSpecsGeneralStateTestsDirectory;

pub use crate::environment::Environment;
pub use crate::filters::Filters;
pub use crate::summary::Summary;
pub use crate::test_suits::ethereum_general_state::EthereumGeneralStateTestsDirectory;
pub use crate::test_suits::Collection;
pub use crate::vm::zk_ee::ZKsyncOS;
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
    /// Optional path to the mutated tests directory
    pub mutation_path: Option<String>,
    pub run_spec_tests: bool,
}

impl EvmTester {
    /// The General state transition tests directory.
    const GENERAL_STATE_TESTS: &'static str = "ethereum-tests/GeneralStateTests";
    const GENERAL_STATE_TESTS_FILLER: &'static str = "ethereum-tests/src/GeneralStateTestsFiller";

    const EXECUTION_SPECS_GENERAL_STATE_TESTS: &'static str = "ethereum-fixtures/state_tests";
}

impl EvmTester {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(
        summary: Arc<Mutex<Summary>>,
        filters: Filters,
        workflow: Workflow,
        mutation_path: Option<String>,
        run_spec_tests: bool,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            summary,
            filters,
            workflow,
            mutation_path,
            run_spec_tests,
        })
    }

    ///
    /// Runs all tests on ZKsync OS.
    ///
    pub fn run_zksync_os(self, vm: ZKsyncOS, run_mutation_tests: bool) -> anyhow::Result<()> {
        let tests = self.all_tests(Environment::ZKsyncOS)?;
        let vm = Arc::new(vm);
        let _: Vec<()> = tests
            .into_par_iter()
            .map(|mut test| {
                let mutants = test.mutants;
                test.mutants = vec![];

                test.run_zksync_os(
                    self.summary.clone(),
                    vm.clone(),
                    matches!(self.workflow, Workflow::Bench),
                );

                if run_mutation_tests {
                    for mutant in mutants {
                        mutant.run_zksync_os(
                            self.summary.clone(),
                            vm.clone(),
                            matches!(self.workflow, Workflow::Bench),
                        );
                    }
                }
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

        if self.run_spec_tests {
            tests.extend(
                self.directory::<EthereumExecutionSpecsGeneralStateTestsDirectory>(
                    Self::EXECUTION_SPECS_GENERAL_STATE_TESTS,
                    "", // don't need fillers here
                    environment,
                )?,
            );
        }

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
            self.mutation_path.clone(),
        )
        .map_err(|error| anyhow::anyhow!("Failed to read the tests directory `{path}`: {error}"))
    }
}
