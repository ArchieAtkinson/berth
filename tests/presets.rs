use berth::presets::Preset;
use indoc::indoc;

#[test]
fn basic_preset_file() {
    let content = r#"
        [env.Env1]
        image = "image1"
        exec_cmds = ["command1", "command2"]
        init_cmd = "init1"
        user = "user"

        [env.Env2]
        image = "image2"
        mounts = ["/my/dir:/their/dir"]
        init_cmd = "init2"
    "#;
    let preset = Preset::new(&content).unwrap();

    assert!(preset.env.contains_key("Env1"));
    assert!(preset.env.contains_key("Env2"));

    assert_eq!(preset.env.get("Env1").unwrap().image, "image1");
    assert_eq!(preset.env.get("Env2").unwrap().image, "image2");

    assert_eq!(
        preset.env.get("Env1").unwrap().exec_cmds,
        Some(vec!["command1".to_string(), "command2".to_string()])
    );
    assert_eq!(preset.env.get("Env2").unwrap().exec_cmds, None);

    assert_eq!(preset.env.get("Env1").unwrap().mounts, None);
    assert_eq!(
        preset.env.get("Env2").unwrap().mounts,
        Some(vec!["/my/dir:/their/dir".to_string()])
    );

    assert_eq!(preset.env.get("Env1").unwrap().init_cmd, "init1");
    assert_eq!(preset.env.get("Env2").unwrap().init_cmd, "init2");

    assert_eq!(
        preset.env.get("Env1").unwrap().user,
        Some("user".to_string())
    );
    assert_eq!(preset.env.get("Env2").unwrap().user, None);
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
