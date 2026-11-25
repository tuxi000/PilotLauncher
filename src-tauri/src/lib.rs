//! PilotLauncher 后端入口。

// 部分工具函数 / 字段只在特定流程中使用，避免 dead_code 警告刷屏
#![allow(dead_code)]

mod auth;
mod commands;
mod community;
mod download;
mod instance;
mod java;
mod launch;
mod minecraft;
mod modloader;
mod paths;
mod settings;
mod state;
mod system;
mod tasks;
mod terracotta;
mod util;

use std::sync::Arc;

use tauri::Manager;

pub fn run() {
    let app = tauri::Builder::default()
        // 文件选择（导入本地整合包 / 地图）
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let core = state::Core::init(app.handle().clone()).map_err(|e| format!("{e:#}"))?;
            let settings = core.settings();
            app.manage(Arc::new(core));
            if let Some(win) = app.get_webview_window("main") {
                if let Err(e) = commands::apply_window_effect(&win, &settings.window_effect, &settings.theme) {
                    eprintln!("{e:#}");
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::get_system_info,
            commands::get_memory_status,
            commands::get_version_manifest,
            commands::list_instances,
            commands::create_instance,
            commands::rename_instance,
            commands::delete_instance,
            commands::update_instance,
            commands::repair_instance,
            commands::select_instance,
            commands::list_saves,
            commands::open_instance_dir,
            commands::list_tasks,
            commands::cancel_task,
            commands::clear_finished_tasks,
            commands::list_accounts,
            commands::add_offline_account,
            commands::check_auth_server,
            commands::add_yggdrasil_account,
            commands::remove_account,
            commands::select_account,
            commands::list_java,
            commands::inspect_java,
            commands::launch_game,
            commands::kill_game,
            commands::get_game_log,
            commands::running_games,
            commands::open_path,
            commands::open_url,
            commands::read_image_data_url,
            commands::list_loader_versions,
            commands::install_loader,
            commands::community_search,
            commands::community_categories,
            commands::community_project,
            commands::community_versions,
            commands::community_plan,
            commands::community_install,
            commands::community_install_modpack,
            commands::inspect_archive,
            commands::import_modpack,
            commands::import_world,
            commands::get_missing_files,
            commands::dismiss_missing_files,
            commands::list_instance_resources,
            commands::set_resource_enabled,
            commands::delete_instance_resource,
            commands::reveal_instance_resource,
            commands::terracotta_status,
            commands::terracotta_watch,
            commands::terracotta_install,
            commands::terracotta_start,
            commands::terracotta_stop,
            commands::terracotta_restart,
            commands::terracotta_host,
            commands::terracotta_join,
            commands::terracotta_leave,
            commands::terracotta_log,
        ])
        .build(tauri::generate_context!())
        .expect("PilotLauncher 启动失败");

    app.run(|handle, event| {
        // 启动器退出时关闭本启动器启动的联机组件（关闭房间 → 正常退出 → 结束子进程）
        if let tauri::RunEvent::Exit = event {
            if let Some(core) = handle.try_state::<Arc<state::Core>>() {
                core.terracotta.shutdown_blocking(&core.paths);
            }
        }
    });
}
