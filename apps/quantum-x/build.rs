fn main() {
    println!("cargo:rerun-if-changed=../../product.json");
    let icon = "../../assets/icons/quantum-dynamics/QUANTUM_Dynamics.ico";
    println!("cargo:rerun-if-changed={icon}");
    let identity: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("../../product.json").expect("product identity"),
    )
    .expect("valid product identity JSON");
    for key in [
        "name",
        "repository",
        "windows_executable",
        "development_executable",
        "project_extension",
    ] {
        let value = identity[key].as_str().expect("identity string");
        assert!(
            !value.is_empty() && !value.contains(['"', '\\', '\n', '\r']),
            "invalid identity resource string"
        );
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let numeric = format!("{},0", version.replace('.', ","));
    let name = identity["name"].as_str().unwrap();
    let filename = identity["windows_executable"].as_str().unwrap();
    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    // Stage the owner's ICO unchanged; ordinary builds never invoke Blender or
    // regenerate artwork. Resource 101 is also loaded by qx-rendering::identity.
    let staged_icon = out_dir.join("quantum-dynamics.ico");
    std::fs::copy(icon, &staged_icon).expect("stage approved QUANTUM Dynamics icon");
    let icon_path = staged_icon
        .to_str()
        .expect("UTF-8 icon path")
        .replace('\\', "/");
    let rc = format!(
        r#"
#pragma code_page(65001)
#include <winver.h>
101 ICON "{icon_path}"
1 VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
 BLOCK "StringFileInfo"
 BEGIN
  BLOCK "040904b0"
  BEGIN
   VALUE "FileDescription", "{name}\0"
   VALUE "ProductName", "{name}\0"
   VALUE "FileVersion", "{version}\0"
   VALUE "ProductVersion", "{version}\0"
   VALUE "InternalName", "quantum-x\0"
   VALUE "OriginalFilename", "{filename}\0"
  END
 END
 BLOCK "VarFileInfo"
 BEGIN
  VALUE "Translation", 0x0409, 1200
 END
END
"#
    );
    let path = out_dir.join("product.rc");
    std::fs::write(&path, rc).expect("write Windows icon and version resources");
    embed_resource::compile(&path, embed_resource::NONE)
        .manifest_required()
        .expect("Windows product icon/metadata resource compilation failed");
}
