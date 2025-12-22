use std::{fs, path::Path};

use miette::{Diagnostic, Result};
use serde::Deserialize;
use thiserror::Error;

use crate::UnexpectedExt;

#[derive(Debug, Error, PartialEq, Diagnostic)]
pub enum DevContError {
    #[error("Bad devcontainer.json path '{0}'\n{1}")]
    #[diagnostic(code(devcontainer::path))]
    Path(String, String),
    #[error("Malformed devcontainer.json {0}")]
    #[diagnostic(code(devcontainer::parsing))]
    Parsing(String),
    #[error("Malformed 'workspaceMount': {0}\n{1}")]
    #[diagnostic(code(devcontainer::json))]
    WorkspaceMount(String, String),
    #[error("Failed to find '${{localWorkspaceFolder}}': {0}\n{1}")]
    #[diagnostic(code(devcontainer::json))]
    WorkspaceFolder(String, String),
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
        let mut content = fs::read_to_string(&json_path).map_err(|e| {
            DevContError::Path(
                json_path
                    .as_ref()
                    .to_str()
                    .unwrap_or("<INVALID_PATH>")
                    .to_owned(),
                e.to_string(),
            )
        })?;

        content = Self::replace_envs(&json_path, &content)?;

        let mut metadata: Metadata =
            serde_json5::from_str(&content).map_err(|e| DevContError::Parsing(e.to_string()))?;

        metadata = Self::parse_mount(metadata)?;
        metadata = Self::fix_build(&json_path, metadata)?;
        Ok(metadata)
    }

    fn replace_envs(json_path: impl AsRef<Path>, content: &str) -> Result<String> {
        let path = json_path.as_ref().to_path_buf();
        eprintln!("PATH: {:?}", path);
        let workspace_dir = path
            .ancestors()
            .nth(2) // Get workspace folder
            .ok_or(DevContError::WorkspaceFolder(
                path.to_str().unexpected()?.to_owned(),
                "Unable to find workspace in path".to_string(),
            ))?
            .to_str()
            .map(|s| s.to_owned())
            .unexpected()?;

        let json = content.replace("${localWorkspaceFolder}", &workspace_dir);

        Ok(json)
    }

    fn parse_mount(mut metadata: Metadata) -> Result<Metadata> {
        let Some(mount) = metadata.workspace_mount.clone() else {
            return Ok(metadata);
        };

        let split: Vec<String> = mount.split(',').map(|f| f.to_owned()).collect();

        let source = split[0]
            .strip_prefix("source=")
            .ok_or(DevContError::WorkspaceMount(
                mount.to_owned(),
                "No 'source' provided".to_string(),
            ))?
            .to_owned();

        let target = split[1]
            .strip_prefix("target=")
            .ok_or(DevContError::WorkspaceMount(
                mount.to_owned(),
                "No 'target' provided".to_string(),
            ))?;

        log::warn!("Assuming mount type is 'bind'");
        let mount = format!("-v {source}:{target}");

        metadata.workspace_mount = Some(mount);
        Ok(metadata)
    }

    fn fix_build(json_path: impl AsRef<Path>, mut metadata: Metadata) -> Result<Metadata> {
        let Some(ref mut build) = metadata.build else {
            return Ok(metadata);
        };

        let dir = json_path.as_ref().parent().unexpected()?;

        if let Some(ref mut dockerfile) = build.dockerfile {
            let mut dockerfile_path = dir.to_path_buf();
            dockerfile_path.push(&dockerfile);
            *dockerfile = dockerfile_path.to_str().unexpected()?.to_owned();
        };

        if let Some(ref mut context) = build.context {
            let mut dockerfile_path = dir.to_path_buf();
            dockerfile_path.push(&context);
            *context = dockerfile_path.to_str().unexpected()?.to_owned();
        };

        Ok(metadata)
    }
}

#[cfg(test)]
mod test {

    use std::{fs::File, io::Write};

    use tempfile::{NamedTempFile, TempDir};

    use super::*;

    #[test]
    fn invalid_path() {
        let report = Metadata::new("").unwrap_err();

        assert!(matches!(
            report.downcast_ref::<DevContError>(),
            Some(DevContError::Path(_, _))
        ));
    }

    #[test]
    fn malform_json() {
        let data = r#"
            {
                name 
            }            
        "#;

        let mut json = NamedTempFile::new().unwrap();
        json.write(data.as_bytes()).unwrap();

        let report = Metadata::new(json.path()).unwrap_err();

        assert!(matches!(
            report.downcast_ref::<DevContError>(),
            Some(DevContError::Parsing(_))
        ));
    }

    #[test]
    fn mount_no_source() {
        let data = r#"
            {
                "workspaceMount": "Source,target=Target,mount=bind",
            }
        "#;

        let mut json = NamedTempFile::new().unwrap();
        json.write(data.as_bytes()).unwrap();

        let report = Metadata::new(json.path()).unwrap_err();

        assert!(matches!(
            report.downcast_ref::<DevContError>(),
            Some(DevContError::WorkspaceMount(_, _))
        ));
    }

    #[test]
    fn mount_no_target() {
        let data = r#"
            {
                "workspaceMount": "source=Source,mount=bind",
            }
        "#;

        let mut json = NamedTempFile::new().unwrap();
        json.write(data.as_bytes()).unwrap();

        let report = Metadata::new(json.path()).unwrap_err();

        assert!(matches!(
            report.downcast_ref::<DevContError>(),
            Some(DevContError::WorkspaceMount(_, _))
        ));
    }

    #[test]
    fn new_metadata() {
        let data = r#"
            {
                "name": "Name",
                "image": "Image",
                "remoteUser": "RemoteUser",
                "workspaceMount": "source=Source,target=Target,mount=bind",
                "workspaceFolder": "${localWorkspaceFolder}",
                "build": {
                    "dockerfile": "Dockerfile",
                    "context": "Context",
                }
            }            
        "#;

        let tmp = TempDir::new().unwrap();

        let workspace_dir = tmp.path().to_path_buf();

        let mut json_path = workspace_dir.clone();
        json_path.push(".devcontainer");
        fs::create_dir_all(&json_path).unwrap();
        json_path.push("devcontainer.json");

        let mut file = File::create(&json_path).unwrap();
        file.write(data.as_bytes()).unwrap();

        let metadata = Metadata::new(&json_path).unwrap();

        assert_eq!(metadata.name, Some("Name".into()));
        assert_eq!(metadata.image, Some("Image".into()));
        assert_eq!(metadata.remote_user, Some("RemoteUser".into()));
        assert_eq!(metadata.workspace_mount, Some("-v Source:Target".into()));
        assert_eq!(
            metadata.workspace_folder,
            Some(workspace_dir.as_path().to_str().unwrap().to_owned())
        );

        let docker_path = format!(
            "{}/{}",
            json_path.parent().unwrap().to_str().unwrap(),
            "Dockerfile"
        );

        let build = metadata.build.unwrap();

        assert_eq!(build.dockerfile, Some(docker_path));

        let context_path = format!(
            "{}/{}",
            json_path.parent().unwrap().to_str().unwrap(),
            "Context"
        );
        assert_eq!(build.context, Some(context_path));
    }
}
