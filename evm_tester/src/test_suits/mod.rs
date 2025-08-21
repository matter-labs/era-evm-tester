//!
//! The buildable compiler test trait.
//!

pub mod index;
pub mod state_tests;

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
        filters: &Filters,
        environment: Environment,
        mutation_path: Option<String>,
        index_path: &Path,
    ) -> anyhow::Result<Vec<Test>>;
}
