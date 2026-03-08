mod utils;

use dir_diff::is_different;

use crate::utils::TestEnv;

#[test]
fn locked() {
    let env = TestEnv::new("basic");
    env.command().arg("lock").assert().success();
    assert!(!is_different(env.fixture(), env.path()).unwrap());
}

#[test]
fn locked_flag() {
    let env = TestEnv::new("basic");
    env.command().arg("lock").arg("--locked").assert().success();
    assert!(!is_different(env.fixture(), env.path()).unwrap());
}
