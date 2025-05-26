use crate::Environment;

pub mod emulator;
pub mod zk_ee;

pub fn index_for_environment(environment: Environment) -> &'static str {
    match environment {
        Environment::EVMEmulator => emulator::INDEX_PATH,
        Environment::ZKsyncOS => zk_ee::INDEX_PATH,
    }
}
