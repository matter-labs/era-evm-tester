use crate::Environment;

pub mod zk_ee;

pub fn index_for_environment(environment: Environment) -> &'static str {
    match environment {
        Environment::ZKsyncOS => zk_ee::INDEX_PATH,
    }
}
