use berth::presets::Preset;
use indoc::{formatdoc, indoc};
use test::TmpEnvVar;

pub mod test;

#[test]
fn basic_preset_file() {
    let content = r#"
        [env.Env1]
        image = "image1"
        entry_cmd = "init1"

        [env.Env2]
        image = "image2"
        entry_cmd = "init2"
    "#;
    let preset = Preset::new(&content).unwrap();

    assert!(preset.envs.contains_key("Env1"));
    assert!(preset.envs.contains_key("Env2"));

    assert_eq!(preset.envs.get("Env1").unwrap().image, "image1");
    assert_eq!(preset.envs.get("Env2").unwrap().image, "image2");

    assert_eq!(preset.envs.get("Env1").unwrap().entry_cmd, "init1");
    assert_eq!(preset.envs.get("Env2").unwrap().entry_cmd, "init2");
}

#[test]
fn unknown_field() {
    let content = indoc! {r#"
        [env.Env]
        unknown = "Should Fail"
    "#};
    let preset = Preset::new(&content);
    assert!(preset.is_err());
    let err_str = preset.unwrap_err().to_string();
    assert!(err_str.contains("TOML parse error at line 2, column 1"));
    assert!(err_str.contains("unknown field `unknown`"));
}

#[test]
fn env_vars_in_options() {
    let var = TmpEnvVar::new("/dir");
    let content = formatdoc!(
        r#"
        [env.Env]
        image = "image"
        entry_cmd = "cmd"
        create_options = ["${}"]
        exec_options = ["${}"]
        entry_options = ["${}"]
    "#,
        var.name(),
        var.name(),
        var.name()
    );

    let mut preset = Preset::new(&content).unwrap();
    let env = preset.envs.remove("Env").unwrap();
    assert_eq!(&env.create_options.unwrap()[0], &format!("{}", var.value()));
    assert_eq!(&env.exec_options.unwrap()[0], &format!("{}", var.value()));
    assert_eq!(&env.entry_options.unwrap()[0], &format!("{}", var.value()));
}
