pub mod commands;
pub mod helpers;

use self::helpers::{Answer, Prompts};

/// Resolves the pending prompt `id` with the user's `values`, or `None` if cancelled.
pub fn answer_prompt(prompts: &Prompts, id: u32, values: Answer) -> Result<(), String> {
    let tx = prompts
        .pending
        .lock()
        .unwrap()
        .remove(&id)
        .ok_or_else(|| format!("No pending prompt #{id}"))?;
    // The requester may have given up already, nothing left to do then
    let _ = tx.send(values);
    Ok(())
}
