//!
//! The evm tester executable.
//!

pub(crate) mod arguments;

use std::time::Instant;

use colored::Colorize;

use self::arguments::Arguments;

/// The rayon worker stack size.
const RAYON_WORKER_STACK_SIZE: usize = 16 * 1024 * 1024;

///
/// The application entry point.
///
fn main() {
    let exit_code = match main_inner(Arguments::new()) {
        Ok(()) => era_compiler_common::EXIT_CODE_SUCCESS,
        Err(error) => {
            eprintln!("{error:?}");
            era_compiler_common::EXIT_CODE_FAILURE
        }
    };
    std::process::exit(exit_code);
}

///
/// The entry point wrapper used for proper error handling.
///
fn main_inner(arguments: Arguments) -> anyhow::Result<()> {
    let mut thread_pool_builder = rayon::ThreadPoolBuilder::new();
    if let Some(threads) = arguments.threads {
        thread_pool_builder = thread_pool_builder.num_threads(threads);
    }
    thread_pool_builder
        .stack_size(RAYON_WORKER_STACK_SIZE)
        .build_global()
        .expect("Thread pool configuration failure");

    let summary = evm_tester::Summary::new(arguments.verbosity, arguments.quiet).wrap();

    let filters = evm_tester::Filters::new(arguments.paths, arguments.groups, arguments.labels);

    let evm_tester = evm_tester::EvmTester::new(summary.clone(), filters, arguments.workflow)?;

    let environment = match arguments.environment {
        Some(environment @ evm_tester::Environment::EVMEmulator) => environment,
        Some(environment @ evm_tester::Environment::ZkOS) => environment,
        None => evm_tester::Environment::EVMEmulator,
    };

    let run_time_start = Instant::now();
    println!(
        "     {} tests with {} worker threads",
        "Running".bright_green().bold(),
        rayon::current_num_threads(),
    );

    match environment {
        evm_tester::Environment::EVMEmulator => {
            let vm = evm_tester::EraVM::new(era_compiler_common::Target::EVM)?;

            evm_tester.run_evm_interpreter::<evm_tester::EraVMSystemContractDeployer, true>(vm)
        }

        evm_tester::Environment::ZkOS => {
            let vm = evm_tester::ZkOS::new();
            evm_tester.run_zk_os(vm)
        }
    }?;

    let summary = evm_tester::Summary::unwrap_arc(summary);
    print!("{summary}");
    println!(
        "    {} running tests in {}m{:02}s",
        "Finished".bright_green().bold(),
        run_time_start.elapsed().as_secs() / 60,
        run_time_start.elapsed().as_secs() % 60,
    );

    if !summary.is_successful() {
        anyhow::bail!("");
    }

    Ok(())
}

#[cfg(test)]
mod tests {

    use crate::arguments::Arguments;

    #[test]
    fn test_manually() {
        std::env::set_current_dir("..").expect("Change directory failed");

        let arguments = Arguments {
            verbosity: false,
            quiet: false,
            paths: vec!["tests/solidity/simple/default.sol".to_owned()],
            groups: vec![],
            labels: vec![],
            threads: Some(1),
            environment: None,
            workflow: evm_tester::Workflow::BuildAndRun,
        };

        crate::main_inner(arguments).expect("Manual testing failed");
    }
}
