//! Installed projects only: no generated rides, thumbnails or template content.
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(super) struct Projects {
    pub samples: Vec<PathBuf>,
    pub templates: Vec<PathBuf>,
    pub issues: Vec<String>,
}
impl Projects {
    pub fn discover(assets: &Path) -> Self {
        let mut value = Self::default();
        value.samples = scan(&assets.join("projects/samples"), &mut value.issues);
        value.templates = scan(&assets.join("projects/templates"), &mut value.issues);
        // Source-checkout examples are existing numerical fixtures. Never fall
        // back to checkout content for a portable distribution with missing assets.
        let checkout = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        if assets
            .canonicalize()
            .ok()
            .is_some_and(|p| Some(p) == checkout.canonicalize().ok())
        {
            value
                .samples
                .extend(scan(&assets.join("../examples"), &mut value.issues));
        }
        value.samples.sort();
        value.samples.dedup();
        value
    }
}
fn scan(directory: &Path, issues: &mut Vec<String>) -> Vec<PathBuf> {
    let items = match std::fs::read_dir(directory) {
        Ok(items) => items,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return vec![],
        Err(e) => {
            issues.push(format!("{}: {e}", directory.display()));
            return vec![];
        }
    };
    let mut paths = Vec::new();
    for item in items.take(1025) {
        if paths.len() == 256 {
            issues.push(format!(
                "{}: showing at most 256 projects",
                directory.display()
            ));
            break;
        }
        match item {
            Ok(item)
                if item.file_type().is_ok_and(|t| t.is_file())
                    && super::startup::is_project_path(&item.path()) =>
            {
                paths.push(item.path())
            }
            Err(e) => issues.push(e.to_string()),
            _ => {}
        }
    }
    paths.sort();
    paths
}
