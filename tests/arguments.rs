use assert_cmd::Command;

const BINARY: &str = env!("CARGO_PKG_NAME");

// #[test]
// fn arguments() {
//     let mut cmd = Command::cargo_bin(BINARY).unwrap();
//     let assert = cmd.args(["--config-file", "sample", "env_name"]).assert();
//     assert.success();
// }
