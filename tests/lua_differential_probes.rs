use std::process::Command;

#[test]
#[ignore = "requires the pinned C server binary and a built Rust server binary"]
fn runtime_surface_matches_the_pinned_c_server() {
    let root = crate::support::repository_root();
    let status = Command::new("python3")
        .arg(root.join("tools/lua_parity_probe.py"))
        .arg("--probe")
        .arg(root.join("tests/fixtures/lua-probes/runtime_surface.lua"))
        .status()
        .unwrap();
    assert!(status.success());
}
