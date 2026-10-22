//! Embeds a minimal `Info.plist` in the executable, so that `cargo run`
//! builds, which have no bundle, are called Dame in the menu bar instead of
//! dame-macos. `scripts/bundle-macos.sh` writes the full one into the bundle,
//! which takes precedence.

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let plist = PathBuf::from(env::var_os("OUT_DIR").expect("set by cargo")).join("Info.plist");
    let version = env::var("CARGO_PKG_VERSION").expect("set by cargo");
    fs::write(
        &plist,
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key>
	<string>Dame</string>
	<key>CFBundleShortVersionString</key>
	<string>{version}</string>
</dict>
</plist>
"#
        ),
    )
    .expect("cannot write Info.plist");
    println!(
        "cargo:rustc-link-arg-bins=-Wl,-sectcreate,__TEXT,__info_plist,{}",
        plist.display()
    );
    println!("cargo:rerun-if-changed=build.rs");
}
