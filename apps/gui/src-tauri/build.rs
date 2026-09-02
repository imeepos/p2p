fn main() {
    // debug 编译下 codegen 仍会读 frontendDist 目录；前端未构建时补空目录避免首编译失败
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dist = manifest.join("../dist");
    if !dist.exists() {
        let _ = std::fs::create_dir_all(dist);
    }
    tauri_build::build()
}
