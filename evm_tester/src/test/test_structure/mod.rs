use std::collections::HashMap;

use env_section::EnvSection;
use info_section::InfoSection;
use post_state::PostState;
use pre_state::PreState;
use transaction_section::TransactionSection;

use serde::Deserialize;

pub mod env_section;
pub mod info_section;
pub mod post_state;
pub mod pre_state;
pub mod transaction_section;

#[derive(Debug, Deserialize, Clone)]
pub struct TestStructure {
    pub _info: InfoSection,
    pub env: EnvSection,
    pub post: HashMap<String, Vec<PostState>>,
    pub pre: PreState,
    pub transaction: TransactionSection,
}
