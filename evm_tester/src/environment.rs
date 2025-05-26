//!
//! The tester environment to run tests on.
//!

///
/// The tester environment to run tests on.
///
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize)]
pub enum Environment {
    /// The EraVM-based EVM emulator.
    EVMEmulator,
    ZKsyncOS,
}

impl std::str::FromStr for Environment {
    type Err = anyhow::Error;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        match string {
            "EVMEmulator" => Ok(Self::EVMEmulator),
            "ZKsyncOS" => Ok(Self::ZKsyncOS),
            string => anyhow::bail!(
                "Unknown environment `{}`. Supported environments: {:?}",
                string,
                vec![Self::EVMEmulator]
                    .into_iter()
                    .map(|element| element.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
        }
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EVMEmulator => write!(f, "EVMEmulator"),
            Self::ZKsyncOS => write!(f, "ZKsync OS"),
        }
    }
}
