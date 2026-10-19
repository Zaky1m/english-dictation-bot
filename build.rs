// Embeds every `texts/<code>.toml` so adding a language needs no Rust change.
use std::{env, fs, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=texts");
    let dir = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("texts");
    let mut files: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    files.sort();
    let mut out = String::from("pub const TEXT_FILES: &[(&str, &str)] = &[\n");
    for path in files {
        let code = path.file_stem().unwrap().to_str().unwrap();
        out.push_str(&format!(
            "    ({code:?}, include_str!({:?})),\n",
            path.display().to_string()
        ));
    }
    out.push_str("];\n");
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("text_files.rs"),
        out,
    )
    .unwrap();
}
