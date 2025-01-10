//!
//! The evm tester summary element.
//!

pub mod outcome;

use colored::Colorize;

use self::outcome::passed_variant::PassedVariant;
use self::outcome::Outcome;

///
/// The evm tester summary element.
///
#[derive(Debug)]
pub struct Element {
    /// The test name.
    pub name: String,
    /// The test outcome.
    pub outcome: Outcome,
}

impl Element {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(name: String, outcome: Outcome) -> Self {
        Self { name, outcome }
    }

    ///
    /// Prints the element.
    ///
    pub fn print(&self, verbosity: bool) -> Option<String> {
        match self.outcome {
            Outcome::Passed { .. } if !verbosity => return None,
            Outcome::Ignored => return None,
            _ => {}
        }

        let outcome = match self.outcome {
            Outcome::Passed { .. } => "PASSED".green(),
            Outcome::Failed { .. } => "FAILED".bright_red(),
            Outcome::Invalid { .. } => "INVALID".red(),
            Outcome::Panicked { .. } => "PANICKED".bright_magenta(),
            Outcome::Ignored => "IGNORED".bright_black(),
        };

        let details = match self.outcome {
            Outcome::Passed {
                ref variant,
                ref group,
            } => {
                let mut details = Vec::new();
                if let PassedVariant::Deploy { size, .. } = variant {
                    details.push(format!("size {size}").bright_white().to_string())
                };
                match variant {
                    PassedVariant::Deploy { cycles, ergs, .. } => {
                        details.push(format!("cycles {cycles}").bright_white().to_string());
                        details.push(format!("ergs {ergs}").bright_white().to_string());
                    }
                    PassedVariant::Runtime { cycles, ergs, gas } => {
                        details.push(format!("cycles {cycles}").bright_white().to_string());
                        details.push(format!("ergs {ergs}").bright_white().to_string());
                        details.push(format!("gas {gas}").bright_white().to_string());
                    }
                    _ => {}
                };
                if let Some(group) = group {
                    details.push(format!("group '{group}'").bright_white().to_string())
                };
                if details.is_empty() {
                    "".to_string()
                } else {
                    format!("({})", details.join(", "))
                }
            }
            Outcome::Failed {
                ref calldata,
                ref exception,
                ref expected,
                ref actual,
            } => {
                let actual_line = if let Some(actual_value) = actual {
                    format!("\n actual: {actual_value}")
                } else {
                    "".to_string()
                };
                if expected.is_some() {
                    format!(
                        "\n Exception: {exception}\n Expected: {}{actual_line} \n (calldata {})",
                        expected.as_ref().unwrap(),
                        calldata,
                    )
                } else {
                    format!("(calldata {})", calldata,)
                }
            }
            Outcome::Invalid {
                ref error,
                ref calldata,
            } => format!("{} (calldata {})", error, calldata),
            _ => String::new(),
        };

        Some(format!("{:>7} {} {}", outcome, self.name, details))
    }
}
