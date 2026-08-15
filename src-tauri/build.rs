fn main() {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest = std::path::Path::new(&out_dir).join("cooked_keys.rs");

    let gb_key = std::env::var("GAMEBRAIN_API_KEY").unwrap_or_default();
    let sgdb_key = std::env::var("STEAMGRID_API_KEY").unwrap_or_default();

    let content = format!(
        "pub const GAMEBRAIN_KEY: &str = {:?};\npub const STEAMGRID_KEY: &str = {:?};\n",
        gb_key, sgdb_key
    );

    std::fs::write(&dest, content).expect("failed to write cooked_keys.rs");

    tauri_build::build()
}
