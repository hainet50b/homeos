use std::path::PathBuf;

pub struct Context {
    base_dir: PathBuf,
}

impl Context {
    pub fn new(base_dir: Option<PathBuf>) -> Self {
        let base_dir = base_dir.unwrap_or_else(|| {
            dirs::data_dir()
                .expect("could not determine data directory")
                .join("homeos")
        });
        Self { base_dir }
    }

    pub fn repos_dir(&self) -> PathBuf {
        self.base_dir.join("repos")
    }

    pub fn default_repo_dir(&self) -> PathBuf {
        self.repos_dir().join("default")
    }

    pub fn packages_dir(&self) -> PathBuf {
        self.default_repo_dir().join("packages")
    }

    pub fn config_path(&self) -> PathBuf {
        self.default_repo_dir().join("homeos.yml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_custom_base_dir() {
        let ctx = Context::new(Some(PathBuf::from("/tmp/test-homeos")));
        assert_eq!(ctx.repos_dir(), Path::new("/tmp/test-homeos/repos"));
        assert_eq!(
            ctx.default_repo_dir(),
            Path::new("/tmp/test-homeos/repos/default")
        );
        assert_eq!(
            ctx.packages_dir(),
            Path::new("/tmp/test-homeos/repos/default/packages")
        );
        assert_eq!(
            ctx.config_path(),
            Path::new("/tmp/test-homeos/repos/default/homeos.yml")
        );
    }

    #[test]
    fn test_default_base_dir() {
        let ctx = Context::new(None);
        let data_dir = dirs::data_dir().unwrap();
        assert_eq!(ctx.base_dir, data_dir.join("homeos"));
    }
}
