//!
//! The enabled test entity description.
//!

use std::path::PathBuf;

///
/// The enabled test entity description.
///
#[derive(Debug, Clone)]
pub struct EnabledTest {
    /// The test path.
    pub path: PathBuf,
    /// The test group.
    pub group: Option<String>,
    pub skip_calldatas: Option<Vec<web3::types::Bytes>>,
    pub skip_cases: Option<Vec<String>>
}

impl EnabledTest {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(
        path: PathBuf,
        group: Option<String>,
        skip_calldatas: Option<Vec<web3::types::Bytes>>,
        skip_cases: Option<Vec<String>>
    ) -> Self {
        Self {
            path,
            group,
            skip_calldatas,
            skip_cases
        }
    }
}
