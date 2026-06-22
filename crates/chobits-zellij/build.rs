use chobits_meta::PLUGIN_PERMISSIONS;

fn main() {
    let out = std::env::var("OUT_DIR").unwrap();

    let rust = format!(
        "&[{}]",
        PLUGIN_PERMISSIONS.iter()
            .map(|p| format!("PermissionType::{}", p))
            .collect::<Vec<_>>()
            .join(",")
    );
    std::fs::write(format!("{}/plugin_permissions.rs", out), rust).unwrap();

    println!("cargo:rerun-if-changed=build.rs");
}
