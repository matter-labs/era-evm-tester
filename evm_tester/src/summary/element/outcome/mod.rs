//!
//! The evm tester summary element outcome.
//!

pub mod passed_variant;

use self::passed_variant::PassedVariant;

///
/// The evm tester summary element outcome.
///
#[derive(Debug)]
pub enum Outcome {
    /// The `passed` outcome.
    Passed {
        /// The outcome variant.
        variant: PassedVariant,
        /// The test group name.
        group: Option<String>,
    },
    /// The `failed` outcome. The output result is incorrect.
    Failed {
        /// The calldata.
        calldata: String,
        exception: bool,
        expected: Option<String>,
        actual: Option<String>,
    },
    /// The `invalid` outcome. The test is incorrect.
    Invalid {
        /// The building error description.
        error: String,
        calldata: String,
    },
    /// The `panicked` outcome. The test execution raised a panic.
    Panicked {
        /// The building error description.
        error: String,
        calldata: String,
    },
    /// The `ignored` outcome. The test is ignored.
    Ignored,
}

impl Outcome {
    ///
    /// A shortcut constructor.
    ///
    pub fn passed(group: Option<String>, variant: PassedVariant) -> Self {
        Self::Passed { group, variant }
    }

    ///
    /// A shortcut constructor.
    ///
    pub fn failed(
        calldata: Vec<u8>,
        exception: bool,
        expected: Option<String>,
        actual: Option<String>,
    ) -> Self {
        Self::Failed {
            calldata: hex::encode(calldata.as_slice()),
            exception,
            expected,
            actual,
        }
    }

    ///
    /// A shortcut constructor.
    ///
    pub fn invalid<S>(error: S, calldata: Vec<u8>) -> Self
    where
        S: ToString,
    {
        Self::Invalid {
            error: error.to_string(),
            calldata: hex::encode(calldata.as_slice()),
        }
    }

    ///
    /// A shortcut constructor.
    ///
    pub fn panicked<S>(error: S, calldata: Vec<u8>) -> Self
    where
        S: ToString,
    {
        Self::Panicked {
            error: error.to_string(),
            calldata: hex::encode(calldata.as_slice()),
        }
    }

    ///
    /// A shortcut constructor.
    ///
    pub fn ignored() -> Self {
        Self::Ignored
    }
}
