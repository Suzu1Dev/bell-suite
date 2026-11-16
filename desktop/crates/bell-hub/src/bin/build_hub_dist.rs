use std::fs;
use std::path::PathBuf;

fn main() {
    let out_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("hub-dist")
        });

    fs::create_dir_all(&out_dir).expect("failed to create hub-dist directory");

    let index_html = bell_hub::render_initial_html();
    fs::write(out_dir.join("index.html"), index_html).expect("failed to write index.html");
    fs::write(out_dir.join("hub.css"), bell_hub::HUB_CSS).expect("failed to write hub.css");
    fs::write(out_dir.join("hub.js"), bell_hub::HUB_BOOT_JS).expect("failed to write hub.js");
    fs::write(out_dir.join("favicon.svg"), bell_hub::FAVICON_SVG)
        .expect("failed to write favicon.svg");
    fs::write(
        out_dir.join("favicon.ico"),
        bell_hub::build_favicon_ico_bytes(),
    )
    .expect("failed to write favicon.ico");

    println!("bell-hub: generated Linear hub assets in {}", out_dir.display());
}
