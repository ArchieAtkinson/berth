use crate::presets::Env;
use envmnt::types::{ExpandOptions, ExpansionType};
use log::info;
use std::process::{exit, Command};

pub fn enter(name: &str, envs: Vec<Env>) -> Result<(), std::io::Error> {
    let env = envs.iter().find(|e| e.name == name).unwrap();

    let mut create_args = vec!["create", "-it", "--name", &env.name];
    if let Some(user) = &env.user {
        create_args.extend_from_slice(&["-u", user]);
    }
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

    info!("{:?}", create_args);

    println!("Creating your container...\n");
    let docker_create = Command::new("docker").args(&create_args).output()?;
    info!("{:#?}", String::from_utf8(docker_create.stdout));
    let status_code = docker_create.status.code();
    match status_code {
        None => {
            eprintln!("Command interrupted by signal, exiting...");
            exit(1);
        }
        Some(0) => (),
        Some(n) => {
            eprintln!(
                "Error creating container with command:\n{} {}\n\n",
                "docker",
                shell_words::join(create_args)
            );
            eprintln!(
                "Error:\n{}",
                String::from_utf8(docker_create.stderr).unwrap()
            );
            exit(n);
        }
    }

    println!("Starting your container...");
    let docker_start = Command::new("docker").args(["start", &env.name]).output()?;
    println!("{:?}", String::from_utf8(docker_start.stdout));
    println!("{:?}", String::from_utf8(docker_start.stderr));

    let cmds = env.exec_cmds.as_ref().unwrap();
    for cmd in cmds {
        let cmd = format!("{} {} {}", "exec", &env.name, cmd);
        let shell_args = shell_words::split(&cmd).unwrap();
        info!("{:?}", shell_args);
        let docker_exec = Command::new("docker").args(shell_args).output()?;
        println!("{:?}", String::from_utf8(docker_exec.stdout));
        println!("{:?}", String::from_utf8(docker_exec.stderr));
    }

    println!("Entering your container...");
    let status = Command::new("docker")
        .args(["exec", "-it", &env.name, &env.init_cmd])
        .status()?;

    std::process::exit(status.code().unwrap_or(0));

    #[allow(unreachable_code)]
    Ok(())
}
