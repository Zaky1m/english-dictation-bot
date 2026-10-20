//! Checks on the static Mini App files that the browser cannot catch for us.
use std::path::Path;

#[test]
fn every_mini_app_module_is_preloaded() {
    let web = Path::new(env!("CARGO_MANIFEST_DIR")).join("web");
    let index = std::fs::read_to_string(web.join("index.html")).unwrap();
    for entry in std::fs::read_dir(&web).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        if name.ends_with(".js") {
            assert!(
                index.contains(&format!(r#"<link rel="modulepreload" href="{name}">"#)),
                "web/index.html does not preload {name}; a missing preload slows the first load"
            );
        }
    }
}
