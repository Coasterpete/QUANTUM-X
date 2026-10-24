use crate::{Project, Result, require};
use qx_style::Catalog;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

pub const MAX_PROJECT_BYTES: u64 = 16 * 1024 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub fn from_json(json: &str, catalog: &Catalog) -> Result<Project> {
    require(
        json.len() as u64 <= MAX_PROJECT_BYTES,
        "project exceeds 16 MiB",
    )?;
    // All persisted structs reject unknown fields. Unsupported data never
    // disappears silently in a read/write round trip.
    let value: serde_json::Value = serde_json::from_str(json)?;
    let version = value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| crate::DocumentError("missing/integer schema version required".into()))?;
    require(
        (1..=3).contains(&version),
        "unsupported project schema version",
    )?;
    require(
        if version < 3 {
            value.get("heartline").is_none()
        } else {
            value.get("heartline").is_some()
        },
        "heartline settings require schema version 3 and must be present in version 3",
    )?;
    let mut project: Project = serde_json::from_str(json)?;
    match project.schema_version {
        1 => {
            // Version 1 has the same original section semantics, but cannot
            // contain representations or units introduced by version 2.
            use qx_geometry::{
                authoring::{Domain, SectionKind},
                transition::Unit,
            };
            require(
                project.sections.iter().all(|s| {
                    s.domain != Domain::Normalized
                        && !matches!(s.kind, SectionKind::Spline { .. })
                        && s.channels()
                            .iter()
                            .all(|(_, c)| c.unit != Unit::RadiansPerNormalized)
                }),
                "version 1 document contains version 2 authoring data",
            )?;
            project.schema_version = crate::SCHEMA_VERSION;
        }
        2 => project.schema_version = crate::SCHEMA_VERSION,
        crate::SCHEMA_VERSION => {}
        _ => {
            return Err(crate::DocumentError(
                "unsupported project schema version".into(),
            ));
        }
    }
    project.validate(catalog)?;
    Ok(project)
}
pub fn to_json(project: &Project, catalog: &Catalog) -> Result<String> {
    project.validate(catalog)?;
    let json = serde_json::to_string_pretty(project)?;
    require(
        json.len() as u64 <= MAX_PROJECT_BYTES,
        "serialized project exceeds 16 MiB",
    )?;
    Ok(json)
}
pub fn load(path: &Path, catalog: &Catalog) -> Result<Project> {
    let file = File::open(path)?;
    require(
        file.metadata()?.len() <= MAX_PROJECT_BYTES,
        "project exceeds 16 MiB",
    )?;
    let mut json = String::new();
    file.take(MAX_PROJECT_BYTES + 1).read_to_string(&mut json)?;
    from_json(&json, catalog)
}
/// Write/sync a sibling temporary file, close it, then replace by rename.
/// Failure before rename leaves the previous project intact. This does not
/// promise directory-entry durability across power loss on every filesystem.
pub fn save(path: &Path, project: &Project, catalog: &Catalog) -> Result<()> {
    let json = to_json(project, catalog)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| crate::DocumentError("save target must have a file name".into()))?;
    let mut temp_name = name.to_os_string();
    temp_name.push(format!(
        ".{}-{}.tmp",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let temporary = parent.join(temp_name);
    // create_new prevents an existing file/symlink from being overwritten.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| -> Result<()> {
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
