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
pub struct EthereumGeneralStateTestsDirectory;

impl EthereumGeneralStateTestsDirectory {
    ///
    /// Reads the Ethereum test index.
    ///
    pub fn read_index(index_path: &Path) -> anyhow::Result<index::FSEntity> {
        let index_data = std::fs::read_to_string(index_path)?;
        let index: index::FSEntity = serde_yaml::from_str(index_data.as_str())?;
        Ok(index)
    }
}

impl Collection for EthereumGeneralStateTestsDirectory {
    fn read_all(
        directory_path: &Path,
        filler_path: &Path,
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

                let file_name = test.path.file_name().unwrap().to_str().unwrap().to_string();

                let dir_name = directory_path.file_name().unwrap();
                let relative_path: PathBuf = test
                    .path
                    .iter() // iterate over path components
                    .skip_while(|s| *s != dir_name)
                    .skip(1)
                    .collect();

                let test_name = remove_suffix(&file_name, ".json").to_string();
                let filler_name_yml = test_name.clone() + "Filler.yml";

                let filler_path = filler_path.join(relative_path.parent().unwrap());
                let filler_path_yml = filler_path.join(filler_name_yml);

                let filler_file;

                let mut is_json = false;
                if std::fs::exists(filler_path_yml.clone()).unwrap() {
                    filler_file = std::fs::read_to_string(filler_path_yml.clone())
                        .unwrap_or_else(|_| panic!("Filler not found: {:?}", filler_path_yml));
                } else {
                    let filler_path_json = filler_path.join(test_name + "Filler.json");

                    if std::fs::exists(filler_path_json.clone()).unwrap() {
                        is_json = true;
                        filler_file = std::fs::read_to_string(filler_path_json.clone())
                            .unwrap_or_else(|_| panic!("Filler not found: {:?}", filler_path_json));
                    } else {
                        return None; // skip
                    }
                }

                Some(Test::from_ethereum_test(
                    &file,
                    &filler_file,
                    is_json,
                    test.skip_calldatas,
                    test.skip_cases,
                    filters,
                    test.path,
                    relative_path,
                    mutation_path.clone(),
                    None,
                ))
            })
            .collect())
    }
}

fn remove_suffix<'a>(s: &'a str, suffix: &str) -> &'a str {
    match s.strip_suffix(suffix) {
        Some(s) => s,
        None => s,
    }
}
