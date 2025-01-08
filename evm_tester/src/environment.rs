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
    ZkOS
}

impl std::str::FromStr for Environment {
    type Err = anyhow::Error;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        match string {
            "EVMEmulator" => Ok(Self::EVMEmulator),
            "ZKOS" => Ok(Self::ZkOS),
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
            Self::ZkOS => write!(f, "ZK OS"),
        }
    }
}
