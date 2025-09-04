use std::collections::HashMap;

use env_section::EnvSection;
use info_section::InfoSection;
use post_state::PostState;
use pre_state::PreState;
use serde::{de::IgnoredAny, Deserialize};
use transaction_section::TransactionSection;

pub mod block_section;
pub mod env_section;
pub mod info_section;
pub mod post_state;
pub mod pre_state;
pub mod transaction_section;

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct StateTestStructure {
    pub _info: InfoSection,
    pub env: EnvSection,
    pub post: HashMap<String, Vec<PostState>>,
    pub pre: PreState,
    pub transaction: TransactionSection,
    config: Option<IgnoredAny>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct BlockchainTestStructure {
    network: Option<IgnoredAny>,
    config: Option<IgnoredAny>,
    genesis_block_header: Option<IgnoredAny>,
    lastblockhash: Option<IgnoredAny>,
    pub pre: PreState,
    pub post_state: HashMap<String, Vec<PostState>>,
    genesis_r_l_p: Option<IgnoredAny>,
    blocks: Option<IgnoredAny>,
    seal_engine: Option<IgnoredAny>,
    pub _info: InfoSection,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum TestStructure {
    State(StateTestStructure),
    Blockchain(BlockchainTestStructure),
}

impl TestStructure {
    pub fn state(&self) -> &StateTestStructure {
        match self {
            Self::State(s) => s,
            Self::Blockchain(_) => panic!("Expected state test, found blockchain test"),
        }
    }
}
