use crate::presets::Env;
use envmnt::types::{ExpandOptions, ExpansionType};
use std::process::Command;

pub fn enter(name: &str, envs: Vec<Env>) -> Result<(), std::io::Error> {
    let env = envs.iter().find(|e| e.name == name).unwrap();

    let mut create_args = vec!["create", "-it", "--name", &env.name];
    let mounts = env.mounts.as_ref().unwrap();
    let mounts_ref: Vec<String> = mounts
        .iter()
        .map(|mount| {
            let mut env_options = ExpandOptions::new();
            env_options.expansion_type = Some(ExpansionType::Unix);
            envmnt::expand(mount, Some(env_options))
        })
        .collect();

    for expanded_mount in &mounts_ref {
        create_args.push("-v");
        create_args.push(expanded_mount.as_str());
    }
    create_args.push(&env.image);

    let docker_create = Command::new("docker").args(create_args).output()?;
    println!("{:#?}", String::from_utf8(docker_create.stdout));
    println!("{:#?}", String::from_utf8(docker_create.stderr));

    let docker_start = Command::new("docker").args(["start", &env.name]).output()?;
    println!("{:?}", String::from_utf8(docker_start.stdout));
    println!("{:?}", String::from_utf8(docker_start.stderr));

    let cmds = env.exec_cmds.as_ref().unwrap();
    let mut args: Vec<&str> = cmds.iter().map(|s| s.as_str()).collect();
    args.insert(0, &env.name);
    args.insert(0, "exec");
    let docker_exec = Command::new("docker").args(args).output()?;
    println!("{:?}", String::from_utf8(docker_exec.stdout));
    println!("{:?}", String::from_utf8(docker_exec.stderr));

    // Use Command to execute docker exec
    let status = Command::new("docker")
        .args(["exec", "-it", &env.name, "/bin/ash"])
        .status()?;

    std::process::exit(status.code().unwrap_or(0));

    #[allow(unreachable_code)]
    Ok(())
}
