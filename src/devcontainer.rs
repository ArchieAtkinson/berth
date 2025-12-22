use std::{fs, path::Path};

use miette::{Diagnostic, Result};
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Diagnostic)]
pub enum JsonError {
    #[error("Bad devcontainer.json path {0}")]
    #[diagnostic(code(devcontainer::parsing))]
    Path(String),
    #[error("Malformed devcontainer.json {0}")]
    #[diagnostic(code(devcontainer::parsing))]
    Parsing(String),
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub name: Option<String>,
    pub remote_user: Option<String>,
    pub workspace_mount: Option<String>,
    pub workspace_folder: Option<String>,
    pub image: Option<String>,
    pub build: Option<Build>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Build {
    pub dockerfile: Option<String>,
    pub context: Option<String>,
}

impl Metadata {
    pub fn new(json_path: impl AsRef<Path>) -> Result<Self> {
        let content = fs::read_to_string(&json_path).map_err(|e| JsonError::Path(e.to_string()))?;
        let mut metadata: Metadata =
            serde_json5::from_str(&content).map_err(|e| JsonError::Parsing(e.to_string()))?;

        Self::fix_mount(&json_path, &mut metadata);
        Self::fix_build(&json_path, &mut metadata);
        Ok(metadata)
    }

    fn fix_mount(json_path: impl AsRef<Path>, metadata: &mut Metadata) {
        let Some(mount) = metadata.workspace_mount.clone() else {
            return;
        };

        let split: Vec<String> = mount.split(',').map(|f| f.to_owned()).collect();
        let mut source = split[0].strip_prefix("source=").unwrap().to_owned();
        if &source == "${localWorkspaceFolder}" {
            let path = json_path.as_ref().to_path_buf();
            source = path
                .ancestors()
                .nth(2) // Get workspace folder
                .and_then(|p| p.to_str())
                .map(|s| s.to_owned())
                .expect("Failed to get grandparent path as string");
        }

        let target = split[1].strip_prefix("target=").unwrap();

        log::warn!("Assuming mount type is 'bind'");
        let mount = format!("-v {source}:{target}");

        metadata.workspace_mount = Some(mount);
    }

    fn fix_build(json_path: impl AsRef<Path>, metadata: &mut Metadata) {
        let Some(ref mut build) = metadata.build else {
            return;
        };

        let devcontainer_root = json_path.as_ref().parent().unwrap();

        if let Some(ref mut dockerfile) = build.dockerfile {
            let mut dockerfile_path = devcontainer_root.to_path_buf();
            dockerfile_path.push(&dockerfile);
            *dockerfile = dockerfile_path.to_str().unwrap().to_owned();
        };

        if let Some(ref mut context) = build.context {
            let mut dockerfile_path = devcontainer_root.to_path_buf();
            dockerfile_path.push(&context);
            *context = dockerfile_path.to_str().unwrap().to_owned();
        };
    }
}

#[cfg(test)]
mod test {

    use std::{fs::File, io::Write};

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn new_metadata() {
        let data = r#"
            {
                "name": "Name",
                "image": "Image",
                "remoteUser": "RemoteUser",
                "workspaceMount": "source=Source,target=Target,mount=bind",
                "workspaceFolder": "WorkspaceFolder",
                "build": {
                    "dockerfile": "Dockerfile",
                    "context": "Context",
                }
            }            
        "#;

        let devcontainer_dir = TempDir::new().unwrap();

        let mut json_path = devcontainer_dir.path().to_path_buf();
        json_path.push("devcontainer.json");

        let mut file = File::create(&json_path).unwrap();
        file.write(data.as_bytes()).unwrap();

        let metadata = Metadata::new(&json_path).unwrap();

        assert_eq!(metadata.name, Some("Name".into()));
        assert_eq!(metadata.image, Some("Image".into()));
        assert_eq!(metadata.remote_user, Some("RemoteUser".into()));
        assert_eq!(metadata.workspace_mount, Some("-v Source:Target".into()));
        assert_eq!(metadata.workspace_folder, Some("WorkspaceFolder".into()));

        let docker_path = format!(
            "{}/{}",
            devcontainer_dir.path().to_str().unwrap(),
            "Dockerfile"
        );

        let build = metadata.build.unwrap();

        assert_eq!(build.dockerfile, Some(docker_path));

        let context_path = format!(
            "{}/{}",
            devcontainer_dir.path().to_str().unwrap(),
            "Context"
        );
        assert_eq!(build.context, Some(context_path));
    }
}
