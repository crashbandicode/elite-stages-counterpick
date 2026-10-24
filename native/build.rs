use std::{env, fs, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-env-changed=COUNTERPICK_IMGUI_DIR");
    if env::var_os("CARGO_FEATURE_ADAPT_NATIVE").is_none() {
        return;
    }
    let directory =
        PathBuf::from(env::var_os("COUNTERPICK_IMGUI_DIR").expect(
            "set COUNTERPICK_IMGUI_DIR to the verified pinned imgui-smash archive directory",
        ));
    let archive = directory.join("libimgui_smash.a");
    let digest = directory.join("libimgui_smash.a.sha256");
    assert_eq!(
        fs::metadata(&archive).expect("pinned ImGui archive").len(),
        13_851_844
    );
    assert_eq!(
        fs::read_to_string(&digest)
            .expect("verified ImGui sidecar")
            .split_whitespace()
            .next(),
        Some("09b296a0f5c67641d229bb4a6d8b170f75281368da3957d666af424476bc2731")
    );
    println!("cargo:rerun-if-changed={}", archive.display());
    println!("cargo:rerun-if-changed={}", digest.display());
    println!("cargo:rustc-link-search=native={}", directory.display());
    println!("cargo:rustc-link-lib=static:-bundle=imgui_smash");
    // Each client owns its compiled ImGui functions/global context pointer;
    // only the existing host creates the context and renderer.
    println!("cargo:rustc-cdylib-link-arg=-Bsymbolic");
    println!("cargo:rustc-cdylib-link-arg=--exclude-libs=libimgui_smash.a");
}
