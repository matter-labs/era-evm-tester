//!
//! The buildable compiler test trait.
//!

pub mod ethereum_execution_specs_general_state;
pub mod ethereum_general_state;
pub mod index;

use crate::filters::Filters;
use crate::test::Test;
use crate::Environment;
use std::path::Path;

///
/// The compiler tests directory trait.
///
pub trait Collection {
    ///
    /// Returns all directory tests.
    ///
    fn read_all(
        directory_path: &Path,
        filler_path: &Path,
        filters: &Filters,
        environment: Environment,
        mutation_path: Option<String>,
    ) -> anyhow::Result<Vec<Test>>;
}
