use std::{fs, path::Path};

fn main() {
    // rust-embed requires the folder to exist at compile time. During `cargo
    // test` or a backend-only build the frontend may not be built yet, so a
    // placeholder keeps the crate compilable; a real build runs after
    // `npm run build` and embeds the real interface.
    let dist = Path::new("../dist");
    if !dist.exists() {
        fs::create_dir_all(dist).expect("cannot create ../dist");
        fs::write(
            dist.join("index.html"),
            "<!doctype html><meta charset=\"utf-8\"><title>Course Studio</title>界面尚未构建，请先运行 npm run build",
        )
        .expect("cannot write placeholder index.html");
    }
    println!("cargo:rerun-if-changed=../dist");
}
