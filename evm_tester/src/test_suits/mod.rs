//!
//! The buildable compiler test trait.
//!

pub mod ethereum_general_state;

use std::path::Path;
use crate::filters::Filters;
use crate::test::Test;

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
    ) -> anyhow::Result<Vec<Test>>;
}
