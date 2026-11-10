use strength_tracker_rs_macros::traced_command;
use tauri::State;

/// @dont_log
#[traced_command(log_payload = false)]
#[tauri::command]
pub async fn answer_prompt(
    prompts: State<'_, super::helpers::Prompts>,
    id: u32,
    values: Option<Vec<String>>,
) -> Result<(), String> {
    super::answer_prompt(&prompts, id, values)
}
