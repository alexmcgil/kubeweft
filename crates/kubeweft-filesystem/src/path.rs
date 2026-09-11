use crate::FilesystemError;

#[derive(Debug, Clone)]
pub(crate) struct FsPath {
    components: Vec<String>,
}

impl FsPath {
    pub(crate) fn parse(path: &str) -> Result<Self, FilesystemError> {
        if !path.starts_with('/') || path.contains('\0') {
            return Err(FilesystemError::InvalidPath);
        }
        if path == "/" {
            return Ok(Self { components: vec![] });
        }
        if path.ends_with('/') {
            return Err(FilesystemError::InvalidPath);
        }
        let components = path[1..]
            .split('/')
            .map(|component| {
                if component.is_empty() || matches!(component, "." | "..") {
                    Err(FilesystemError::InvalidPath)
                } else {
                    Ok(component.to_owned())
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { components })
    }

    pub(crate) fn components(&self) -> &[String] {
        &self.components
    }

    pub(crate) fn split_parent(&self) -> Result<(&[String], &str), FilesystemError> {
        let (name, parent) = self
            .components
            .split_last()
            .ok_or(FilesystemError::InvalidPath)?;
        Ok((parent, name))
    }
}
