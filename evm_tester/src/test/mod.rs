//!
//! The test.
//!

pub mod case;
pub mod filler_structure;
pub mod test_structure;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use era_compiler_common::EVMVersion;
use filler_structure::FillerStructure;
use regex::Regex;
use test_structure::TestStructure;

use crate::summary::Summary;
use crate::test::case::Case;
use crate::vm::eravm::deployers::EraVMDeployer;
use crate::vm::eravm::EraVM;
use crate::Filters;
use crate::ZkOS;

fn wrap_numbers_in_quotes(input: &str) -> String {
    // Match numbers not already inside quotes
    //let re = Regex::new(r#": "?\b(\d+)\b"?"#).unwrap();
    //let res1 = re.replace_all(input, ": \"$1\"").to_string();

    //let re2 = Regex::new(r#""?\b(\d+)\b"?:"#).unwrap();
    //let res2 = re2.replace_all(&res1, "\"$1\":").to_string();

    let re3 = Regex::new(r#"\s((0x)?[0-9a-fA-F]{2,}):"#).unwrap();
    let res3 = re3.replace_all(input, " \"$1\":").to_string();

    let re4 = Regex::new(r#": ((0x)?[0-9a-fA-F]{2,})\b"#).unwrap();
    re4.replace_all(&res3, ": \"$1\"").to_string()
}

///
/// The test.
///
#[derive(Debug)]
pub struct Test {
    /// The test name.
    pub name: String,
    /// The test cases.
    pub cases: Vec<Case>,
    /// The test group.
    group: Option<String>,
    /// The EVM version.
    evm_version: Option<EVMVersion>,
    skipped_calldatas: Option<Vec<web3::types::Bytes>>,
    skipped_cases: Option<Vec<String>>,
    pub path: PathBuf,
    pub mutants: Vec<Test>,
}

impl Test {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(
        name: String,
        cases: Vec<Case>,
        group: Option<String>,
        evm_version: Option<EVMVersion>,
        skipped_calldatas: Option<Vec<web3::types::Bytes>>,
        skipped_cases: Option<Vec<String>>,
        path: PathBuf,
        mutants: Vec<Test>,
    ) -> Self {
        Self {
            name,
            cases,
            group,
            evm_version,
            skipped_calldatas,
            skipped_cases,
            path,
            mutants,
        }
    }

    pub fn from_ethereum_test(
        str: &str,
        filler_str: &str,
        is_json: bool,
        skipped_calldatas: Option<Vec<web3::types::Bytes>>,
        skipped_cases: Option<Vec<String>>,
        filters: &Filters,
        path: PathBuf,
        name_override: Option<String>,
    ) -> Self {
        let cleaned_str = str.replace("0x:bigint ", "");
        let test_structure: HashMap<String, TestStructure> =
            serde_json::from_str(&cleaned_str).unwrap();

        let keys: Vec<_> = test_structure.keys().collect();
        let test_name = keys[0];

        let test_filler_structure: HashMap<String, FillerStructure> = if is_json {
            serde_json::from_str(filler_str).unwrap()
        } else {
            let wrapped_numbers = wrap_numbers_in_quotes(filler_str);
            //fs::write("out.yaml", wrapped_numbers.clone());
            serde_yaml::from_str(&wrapped_numbers).unwrap()
        };

        let test_definition = test_structure.get(keys[0]).expect("Always exists");
        let test_filler = test_filler_structure.get(keys[0]).expect("Always exists");

        let cases = Case::from_ethereum_test(test_definition, test_filler, filters);

        // read mutants
        // filter all files in directory by regexp and run
        let mutation_test_re = Regex::new(r"^(.+)_m\d+\.json").unwrap();

        let test_path = path.clone();
        let mut directory = test_path.clone();
        directory.pop();

        let base_test_name = test_path.file_stem().unwrap().to_str().unwrap();

        let files: Vec<_> = std::fs::read_dir(directory)
            .unwrap()
            .map(|x| x.unwrap())
            .filter(|x| {
                let filename = x.file_name();
                let filename = filename.to_str().unwrap();
                if mutation_test_re.is_match(&filename) {
                    let base_name = mutation_test_re
                        .captures(&filename)
                        .unwrap()
                        .get(1)
                        .unwrap()
                        .as_str();
                    if base_name == base_test_name {
                        return true;
                    }
                }
                false
            })
            .collect();

        let mutants: Vec<_> = files
            .into_iter()
            .map(|file| {
                let test_str = std::fs::read_to_string(file.path()).unwrap();
                Test::from_ethereum_test(
                    &test_str,
                    filler_str,
                    is_json,
                    skipped_calldatas.clone(),
                    skipped_cases.clone(),
                    filters,
                    file.path(),
                    Some(
                        file.path()
                            .file_stem()
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .to_string(),
                    ),
                )
            })
            .collect();

        let name = if let Some(name) = name_override {
            name
        } else {
            test_name.clone()
        };

        Self {
            name,
            cases,
            group: None,
            evm_version: None,
            skipped_calldatas,
            skipped_cases,
            path,
            mutants,
        }
    }

    ///
    /// Runs the test on EVM interpreter.
    ///
    pub fn run_evm_interpreter<D, const M: bool>(self, summary: Arc<Mutex<Summary>>, vm: Arc<EraVM>)
    where
        D: EraVMDeployer,
    {
        for case in self.cases {
            if let Some(filter_calldata) = self.skipped_calldatas.as_ref() {
                if filter_calldata.contains(&case.transaction.data) {
                    Summary::ignored(summary.clone(), case.label);
                    continue;
                }
            }

            if let Some(filter_cases) = self.skipped_cases.as_ref() {
                if filter_cases.contains(&case.label) {
                    Summary::ignored(summary.clone(), case.label);
                    continue;
                }
            }

            let vm = EraVM::clone_with_contracts(vm.clone(), Default::default(), self.evm_version);
            case.run_evm_interpreter::<D, M>(
                summary.clone(),
                vm,
                self.name.clone(),
                self.group.clone(),
            );
        }
    }

    ///
    /// Runs the test on ZK OS.
    ///
    pub fn run_zk_os(self, summary: Arc<Mutex<Summary>>, vm: Arc<ZkOS>, bench: bool) {
        for case in self.cases {
            if let Some(filter_calldata) = self.skipped_calldatas.as_ref() {
                if filter_calldata.contains(&case.transaction.data) {
                    Summary::ignored(summary.clone(), case.label);
                    continue;
                }
            }

            if let Some(filter_cases) = self.skipped_cases.as_ref() {
                if filter_cases.contains(&case.label) {
                    Summary::ignored(summary.clone(), case.label);
                    continue;
                }
            }

            let vm = ZkOS::clone(vm.clone());
            case.run_zk_os(
                summary.clone(),
                vm,
                self.name.clone(),
                self.group.clone(),
                bench,
            );
        }
    }
}
