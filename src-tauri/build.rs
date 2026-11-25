fn main() {
    // 发布构建可通过环境变量内置 CurseForge API Key（GitHub Actions 从仓库 Secrets 注入）
    println!("cargo:rerun-if-env-changed=PILOT_CURSEFORGE_API_KEY");
    tauri_build::build()
}
