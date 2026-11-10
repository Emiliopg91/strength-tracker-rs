use strength_tracker_rs_macros::traced_command;
use tauri::State;

/// Answers the pending prompt `id`, sent by the `prompt_requested` event, with the values entered
/// (in the order the prompt kind defines), or `null` if cancelled. Rejects if no such prompt is
/// pending. Neither logged nor traced with its payload, as it carries credentials.
///
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
