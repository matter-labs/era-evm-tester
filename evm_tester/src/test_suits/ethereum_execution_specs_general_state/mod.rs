//!
//! The Ethereum tests directory.
//!

use std::path::Path;
use std::path::PathBuf;

pub mod platforms;

use platforms::index_for_environment;

use crate::filters::Filters;
use crate::test::Test;
use crate::test_suits::Collection;
use crate::Environment;

use super::index;

///
/// The Ethereum tests directory.
///
pub struct EthereumExecutionSpecsGeneralStateTestsDirectory;

impl EthereumExecutionSpecsGeneralStateTestsDirectory {
    ///
    /// Reads the Ethereum test index.
    ///
    pub fn read_index(index_path: &Path) -> anyhow::Result<index::FSEntity> {
        let index_data = std::fs::read_to_string(index_path)?;
        let index: index::FSEntity = serde_yaml::from_str(index_data.as_str())?;
        Ok(index)
    }
}

/*fn update_index(index_path: &Path, directory_path: &Path) -> anyhow::Result<()> {
    let index = index::FSEntity::index(directory_path)?;
    let _ = std::fs::write(index_path, serde_yaml::to_string(&index)?.as_bytes());

    Ok(())
}*/

impl Collection for EthereumExecutionSpecsGeneralStateTestsDirectory {
    fn read_all(
        directory_path: &Path,
        _filler_path: &Path,
        filters: &Filters,
        environment: Environment,
        mutation_path: Option<String>,
    ) -> anyhow::Result<Vec<Test>> {
        let index_path = PathBuf::from(index_for_environment(environment));

        Ok(Self::read_index(index_path.as_path())?
            .into_enabled_list(directory_path)
            .into_iter()
            .filter_map(|test| {
                let identifier = test.path.to_string_lossy().to_string();

                if !filters.check_case_path(&identifier) {
                    return None;
                }

                if !filters.check_group(&test.group) {
                    return None;
                }

                let file = std::fs::read_to_string(test.path.clone())
                    .unwrap_or_else(|_| panic!("Test not found: {:?}", test.path));

                let dir_name = directory_path.file_name().unwrap();
                let relative_path: PathBuf = test
                    .path
                    .iter() // iterate over path components
                    .skip_while(|s| *s != dir_name)
                    .skip(1)
                    .collect();

                Some(Test::from_ethereum_spec_test(
                    &file,
                    test.skip_calldatas,
                    test.skip_cases,
                    test.skip_names,
                    filters,
                    test.path,
                    relative_path,
                    mutation_path.clone(),
                    None,
                    test.hardfork_override,
                ))
            })
            .flatten()
            .collect())
    }
}
