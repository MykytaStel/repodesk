use std::fs;
use std::path::PathBuf;

const REQUIRED_BUNDLE_ICONS: &[&str] = &[
    "32x32.png",
    "128x128.png",
    "128x128@2x.png",
    "icon.icns",
    "icon.ico",
];

fn main() {
    ensure_bundle_icon_fallbacks();
    tauri_build::build();
}

fn ensure_bundle_icon_fallbacks() {
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR")
            .expect("Cargo must provide CARGO_MANIFEST_DIR to the Tauri build script"),
    );
    let committed_icons = manifest_dir.join("icons");
    let generated_icons = manifest_dir.join("generated").join("repodesk-icons");

    fs::create_dir_all(&generated_icons)
        .expect("failed to create RepoDesk generated icon directory");

    for name in REQUIRED_BUNDLE_ICONS {
        let source = committed_icons.join(name);
        let destination = generated_icons.join(name);
        println!("cargo:rerun-if-changed={}", source.display());

        // `tauri dev` / `tauri build` generate the branded icon set before Cargo
        // starts. Plain Cargo commands used by CI do not run Tauri's
        // beforeBuildCommand, so keep those builds compilable with the committed
        // fallback icons without overwriting a freshly generated branded asset.
        if !destination.exists() {
            fs::copy(&source, &destination).unwrap_or_else(|error| {
                panic!(
                    "failed to provision fallback bundle icon '{}' from '{}': {error}",
                    destination.display(),
                    source.display()
                )
            });
        }
    }
}
