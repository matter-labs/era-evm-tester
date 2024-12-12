//!
//! The evm tester arguments.
//!

use structopt::StructOpt;

///
/// The evm tester arguments.
///
#[derive(Debug, StructOpt)]
#[structopt(name = "evm-tester", about = "EVM Implementations Testing Framework")]
pub struct Arguments {
    /// The logging level.
    #[structopt(short = "v", long = "verbose")]
    pub verbosity: bool,

    /// Suppresses the output completely.
    #[structopt(short = "q", long = "quiet")]
    pub quiet: bool,

    /// Runs only tests whose name contains any string from the specified ones.
    #[structopt(short = "p", long = "path")]
    pub paths: Vec<String>,

    /// Runs only tests from the specified groups.
    #[structopt(short = "g", long = "group")]
    pub groups: Vec<String>,

    /// Sets the number of threads, which execute the tests concurrently.
    #[structopt(short = "t", long = "threads")]
    pub threads: Option<usize>,

    /// Specify the environment to run tests on.
    /// Available arguments: `EVMEmulator`.
    /// The default value is EVMEmulator
    #[structopt(long = "environment")]
    pub environment: Option<evm_tester::Environment>,

    /// Choose between `build` to compile tests only without running, and `run` to compile and run.
    #[structopt(long = "workflow", default_value = "run")]
    pub workflow: evm_tester::Workflow,
}

impl Arguments {
    ///
    /// A shortcut constructor.
    ///
    pub fn new() -> Self {
        Self::from_args()
    }
}
