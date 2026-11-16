fn main() {
    println!("cargo:rerun-if-env-changed=BELL_DESKTOP_EDITION");
    tauri_build::build()
}
