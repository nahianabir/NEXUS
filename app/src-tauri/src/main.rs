// NEXUS binary. The Tauri bootstrap lives here directly (not in the lib) so
// the test suite can link `nexus_core` without dragging in WebView2.
#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod commands;

use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;

use nexus_core::app_state::AppState;

fn main() {
    // Catch any panic during early startup and dump it to a file in the
    // data dir so we can diagnose issues even when the GUI subsystem
    // swallows stderr.
    std::panic::set_hook(Box::new(|info| {
        let dir = std::env::var("APPDATA")
            .map(|p| std::path::PathBuf::from(p).join("ai.nexus.workbench"))
            .unwrap_or_else(|_| std::env::temp_dir().join("ai.nexus.workbench"));
        let _ = std::fs::create_dir_all(&dir);
        let crash = dir.join("crash.log");
        let _ = std::fs::write(
            &crash,
            format!("panic at {}: {}\n", chrono::Utc::now().to_rfc3339(), info),
        );
    }));
    // Catch native access violations (e.g. null deref in C deps) and
    // dump the exception address to the same crash log.
    #[cfg(windows)]
    unsafe {
        extern "system" {
            fn SetUnhandledExceptionFilter(
                f: Option<unsafe extern "system" fn(*mut std::ffi::c_void) -> i32>,
            ) -> Option<unsafe extern "system" fn(*mut std::ffi::c_void) -> i32>;
        }
        unsafe extern "system" fn handler(info: *mut std::ffi::c_void) -> i32 {
            let dir = std::env::var("APPDATA")
                .map(|p| std::path::PathBuf::from(p).join("ai.nexus.workbench"))
                .unwrap_or_else(|_| std::env::temp_dir().join("ai.nexus.workbench"));
            let _ = std::fs::create_dir_all(&dir);
            let crash = dir.join("crash.log");
            let addr = if !info.is_null() { info as usize } else { 0 };
            let _ = std::fs::write(
                &crash,
                format!(
                    "SEGV/AV at {}: exception record at 0x{addr:x}\n",
                    chrono::Utc::now().to_rfc3339()
                ),
            );
            0 // EXCEPTION_CONTINUE_SEARCH
        }
        SetUnhandledExceptionFilter(Some(handler));
    }
    nexus_core::logging::init();

    // Tauri 2 re-uses a Tokio runtime under the hood; the `tauri::async_runtime`
    // alias points at it. We still need a Tokio context available to the
    // synchronous parts of the code (the stream select! inside async commands,
    // and the system monitor). Spawning the entire app inside `tauri::async_runtime::spawn`
    // would deadlock on Tauri internals; instead we install a global single-thread
    // runtime for synchronous code paths and let Tauri's runtime handle async commands.
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, _argv, _cwd| {
            log::info!("single-instance: another launch was suppressed");
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            // Resolve the data directory in priority order:
            //   1. NEXUS_DATA_DIR env var (set by a launcher
            //      or installer).
            //   2. <exe_dir>/data if writable (portable mode).
            //   3. Tauri's app_data_dir() (the default roaming
            //      location).
            // The first time we boot, we write the resolved
            // path into the config so the Settings page can
            // display it. After the first launch the user may
            // change the value from the UI; that requires a
            // restart to take effect.
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("resolve app data dir: {e}"))?;
            std::fs::create_dir_all(&app_data_dir).map_err(|e| format!("create app data dir: {e}"))?;

            // First launch: pick a sensible default. We try
            // the candidate list and fall back to the app
            // data dir.
            let mut chosen = None;
            for c in nexus_core::data_dir::candidates() {
                if nexus_core::data_dir::validate(&c.path).is_ok() {
                    chosen = Some(c);
                    break;
                }
            }
            let resolved = if let Some(c) = chosen {
                c.path
            } else {
                PathBuf::from(app_data_dir)
            };
            std::fs::create_dir_all(&resolved).map_err(|e| format!("create data dir: {e}"))?;

            let state = AppState::initialize(&resolved)?;
            app.manage(Arc::new(state));
            log::info!("NEXUS booted; data_dir={}", resolved.display());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::app::list_sidebar,
            commands::app::dashboard_summary,
            commands::app::open_main_window,
            commands::app::app_data_dir_info,
            commands::app::app_data_dir_validate,
            commands::app::app_data_dir_migrate,
            commands::app::app_data_dir_open,
            commands::models::providers_probe,
            commands::models::providers_set_active,
            commands::models::providers_start_lmstudio,
            commands::models::providers_stop_lmstudio,
            commands::models::models_list,
            commands::models::models_upsert,
            commands::models::models_delete,
            commands::models::models_set_active,
            commands::models::models_test_prompt,
            commands::models::models_test_embedding,
            commands::models::cancel_inflight,
            commands::chat::chat_list_conversations,
            commands::chat::chat_create_conversation,
            commands::chat::chat_get_conversation,
            commands::chat::chat_rename_conversation,
            commands::chat::chat_archive_conversation,
            commands::chat::chat_delete_conversation,
            commands::chat::chat_list_messages,
            commands::chat::chat_set_system_prompt,
            commands::chat::chat_request_list_active,
            commands::chat::chat_request_cancel,
            commands::chat::chat_send,
            commands::chat::chat_read_attachment,
            commands::sandbox::sandbox_run_javascript,
            commands::sandbox::sandbox_status,
            commands::vault::vault_list_sources,
            commands::vault::vault_register_source,
            commands::vault::vault_delete_source,
            commands::vault::vault_list_documents,
            commands::vault::vault_get_document,
            commands::vault::vault_delete_document,
            commands::vault::vault_ingest_source,
            commands::vault::vault_search,
            commands::vault::vault_list_citations,
            commands::vault::vault_embed_backfill,
            commands::vault::vault_stats,
            commands::projects::workspaces_list,
            commands::projects::workspaces_create,
            commands::projects::workspaces_rename,
            commands::projects::workspaces_delete,
            commands::projects::projects_list,
            commands::projects::projects_get,
            commands::projects::projects_create,
            commands::projects::projects_update,
            commands::projects::projects_archive,
            commands::projects::projects_delete,
            commands::projects::projects_summaries,
            commands::projects::notes_list,
            commands::projects::notes_get,
            commands::projects::notes_create,
            commands::projects::notes_update,
            commands::projects::notes_delete,
            commands::projects::tasks_list,
            commands::projects::tasks_due_in_range,
            commands::projects::tasks_get,
            commands::projects::tasks_create,
            commands::projects::tasks_update,
            commands::projects::tasks_set_status,
            commands::projects::tasks_delete,
            commands::projects::memory_list,
            commands::projects::memory_create,
            commands::projects::memory_update,
            commands::projects::memory_approve,
            commands::projects::memory_delete,
            commands::projects::ui_state_get,
            commands::projects::ui_state_set,
            commands::projects::ui_state_unset,
            commands::projects::get_active_project,
            commands::projects::set_active_project,
            commands::memory_v2::memories_v2_list,
            commands::memory_v2::memories_v2_get,
            commands::memory_v2::memories_v2_create,
            commands::memory_v2::memories_v2_update,
            commands::memory_v2::memories_v2_approve,
            commands::memory_v2::memories_v2_supersede,
            commands::memory_v2::memories_v2_delete,
            commands::memory_v2::memories_v2_search,
            commands::memory_v2::memory_save_explicit,
            commands::memory_v2::memories_v2_categories,
            commands::memory_v2::memories_v2_migrate_from_v1,
            commands::memory_v2::memory_evidence_list,
            commands::memory_v2::memory_evidence_add,
            commands::memory_v2::memory_evidence_verify,
            commands::memory_v2::memory_evidence_delete,
            commands::memory_v2::memory_candidates_list,
            commands::memory_v2::memory_candidates_propose,
            commands::memory_v2::memory_candidates_approve,
            commands::memory_v2::memory_candidates_reject,
            commands::memory_v2::memory_sessions_list,
            commands::memory_v2::memory_sessions_get,
            commands::memory_v2::memory_sessions_create,
            commands::memory_v2::memory_sessions_delete,
            commands::mempalace::memory_backend_status,
            commands::mempalace::memory_backend_activate_native,
            commands::mempalace::memory_backend_activate_mempalace,
            commands::mempalace::memory_backend_disable,
            commands::mempalace::memory_backend_search,
            commands::mempalace::memory_backend_store,
            commands::mempalace::memory_backend_delete,
            commands::kg::kg_entity_types,
            commands::kg::kg_relationship_types,
            commands::kg::kg_list_entities,
            commands::kg::kg_search,
            commands::kg::kg_get_entity,
            commands::kg::kg_create_entity,
            commands::kg::kg_add_alias,
            commands::kg::kg_delete_entity,
            commands::kg::kg_merge_entities,
            commands::kg::kg_create_relationship,
            commands::kg::kg_set_relationship_status,
            commands::kg::kg_list_relationships_for,
            commands::kg::kg_delete_relationship,
            commands::kg::kg_evidence_for,
            commands::kg::kg_add_evidence,
            commands::kg::kg_delete_evidence,
            commands::kg::kg_neighborhood,
            commands::kg::kg_shortest_path,
            commands::kg::kg_projects_using,
            commands::kg::kg_stats,
            commands::kg::kg_seed_project,
            commands::context::context_preview,
            commands::router::router_capabilities_list,
            commands::router::router_capabilities_get,
            commands::router::router_capabilities_upsert,
            commands::router::router_capabilities_infer,
            commands::router::router_lifecycle_list,
            commands::router::router_lifecycle_transition,
            commands::router::router_route,
            commands::router::router_decisions_recent,
            commands::research::research_project_create,
            commands::research::research_project_get,
            commands::research::research_project_list,
            commands::research::research_project_update,
            commands::research::research_project_delete,
            commands::research::research_paper_add,
            commands::research::research_paper_update,
            commands::research::research_paper_delete,
            commands::research::research_paper_list,
            commands::research::research_claim_add,
            commands::research::research_claim_update,
            commands::research::research_claim_delete,
            commands::research::research_claim_list,
            commands::research::research_matrix_create,
            commands::research::research_matrix_list,
            commands::research::research_matrix_delete,
            commands::research::research_matrix_columns,
            commands::research::research_matrix_column_add,
            commands::research::research_matrix_column_delete,
            commands::research::research_matrix_cells,
            commands::research::research_matrix_cell_set,
            commands::research::research_report_save,
            commands::research::research_report_get,
            commands::research::research_report_list,
            commands::research::research_report_delete,
            commands::research::research_report_build,
            commands::experiments::experiment_create,
            commands::experiments::experiment_get,
            commands::experiments::experiment_list,
            commands::experiments::experiment_update,
            commands::experiments::experiment_delete,
            commands::experiments::experiment_run_create,
            commands::experiments::experiment_run_get,
            commands::experiments::experiment_run_list,
            commands::experiments::experiment_run_update,
            commands::experiments::experiment_run_delete,
            commands::experiments::experiment_metric_history,
            commands::agent::agent_tools_list,
            commands::agent::agent_tool_get,
            commands::agent::agent_tool_set_enabled,
            commands::agent::agent_run_create,
            commands::agent::agent_run_list,
            commands::agent::agent_run_get,
            commands::agent::agent_run_set_status,
            commands::agent::agent_run_delete,
            commands::agent::agent_invocation_create,
            commands::agent::agent_invocation_get,
            commands::agent::agent_invocation_list,
            commands::agent::agent_invocation_recent,
            commands::agent::agent_run_tool,
            commands::agent::agent_invocation_approve,
            commands::agent::agent_invocation_deny,
            commands::intelligence::document_extract_tables,
            commands::intelligence::document_export_csv,
            commands::intelligence::document_diff,
            commands::audit_cmd::audit_recent,
            commands::audit_cmd::audit_clear,
            commands::audit_cmd::audit_prune,
            commands::audit_cmd::audit_prune_by_retention,
            commands::timeline::timeline_ingest_project,
            commands::timeline::timeline_list,
            commands::timeline::timeline_count,
            commands::timeline::timeline_reconstruct,
            commands::timeline::timeline_compare,
            commands::timeline::timeline_superseded,
            commands::timeline::timeline_decisions,
            commands::timeline::timeline_insert_manual,
            commands::timeline::timeline_delete,
            commands::checkpoints::checkpoints_list,
            commands::checkpoints::checkpoints_get,
            commands::checkpoints::checkpoints_create,
            commands::checkpoints::checkpoints_update,
            commands::checkpoints::checkpoints_set_status,
            commands::checkpoints::checkpoints_delete,
            commands::checkpoints::checkpoints_resume,
            commands::checkpoints::checkpoints_list_resumes,
            commands::s3::radar_report,
            commands::s3::radar_counts,
            commands::s3::hypotheses_list,
            commands::s3::hypotheses_get,
            commands::s3::hypotheses_create,
            commands::s3::hypotheses_update,
            commands::s3::hypotheses_decide,
            commands::s3::hypotheses_delete,
            commands::s3::hypotheses_add_evidence,
            commands::s3::hypotheses_list_evidence,
            commands::s3::hypotheses_delete_evidence,
            commands::s3::archaeologist_index,
            commands::s3::archaeologist_head,
            commands::s3::serendipity_report,
            commands::s3::impact_simulate,
            commands::s3::radar_feedback_upsert,
            commands::s3::radar_feedback_delete,
            commands::s3::radar_feedback_list,
            commands::s3::evidence_tribunal_run,
            commands::s3::vault_export_run,
            commands::s3::vault_export_inspect,
            commands::s3::serendipity_report,
            commands::s3::impact_simulate,
        ])
        .run(tauri::generate_context!())
        .expect("error while running NEXUS");
}