use berth::presets::Preset;
use indoc::indoc;

// mod test;
// use test::Test;

#[test]
fn basic_preset_file() {
    let content = r#"
        [[env]]
        name = "Env1"
        image = "image1"
        exec_cmds = ["command1", "command2"]
        init_cmd = "init1"
        user = "user"

        [[env]]
        name = "Env2"
        image = "image2"
        mounts = ["/my/dir:/their/dir"]
        init_cmd = "init2"
    "#;
    let preset = Preset::new(&content);
    assert!(preset.is_ok());

    let preset = preset.unwrap();

    assert_eq!(preset.env[0].name, "Env1");
    assert_eq!(preset.env[1].name, "Env2");

    assert_eq!(preset.env[0].image, "image1");
    assert_eq!(preset.env[1].image, "image2");

    assert_eq!(
        preset.env[0].exec_cmds,
        Some(vec!["command1".to_string(), "command2".to_string()])
    );
    assert_eq!(preset.env[1].exec_cmds, None);

    assert_eq!(preset.env[0].mounts, None);
    assert_eq!(
        preset.env[1].mounts,
        Some(vec!["/my/dir:/their/dir".to_string()])
    );

    assert_eq!(preset.env[0].init_cmd, "init1");
    assert_eq!(preset.env[1].init_cmd, "init2");

    assert_eq!(preset.env[0].user, Some("user".to_string()));
    assert_eq!(preset.env[1].user, None);
}

#[test]
fn unknown_field() {
    let content = indoc! {r#"
        [[env]]
        unknown = "Should Fail"
    "#};
    let preset = Preset::new(&content);
    assert!(preset.is_err());
    let err_str = preset.unwrap_err().to_string();
    assert!(err_str.contains("TOML parse error at line 2, column 1"));
    assert!(err_str.contains("unknown field `unknown`"));
}
