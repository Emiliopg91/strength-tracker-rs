use std::{
    collections::HashMap,
    sync::{
        Mutex,
        atomic::{AtomicU32, Ordering},
    },
};

use futures_util::{FutureExt, future::BoxFuture};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_log::log::{error, info, warn};
use tokio::sync::oneshot;

use crate::{
    connect::ConnectPrompter,
    dto::prompts::{PromptKind, PromptRequest},
    logic::app::show_main_window,
};

type Answer = Option<Vec<String>>;

/// Prompts sent to the frontend that are still waiting for an answer, by id.
#[derive(Default)]
pub struct Prompts {
    next_id: AtomicU32,
    pending: Mutex<HashMap<u32, oneshot::Sender<Answer>>>,
}

/// Asks the frontend for the input described by `kind` and waits for the answer. Returns `None`
/// if the user cancels or the prompt could not be shown.
pub async fn request_prompt<R: Runtime>(
    app: &AppHandle<R>,
    kind: PromptKind,
    prefill: Option<String>,
    detail: Option<String>,
) -> Answer {
    let prompts = app.state::<Prompts>();
    let id = prompts.next_id.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = oneshot::channel();
    prompts.pending.lock().unwrap().insert(id, tx);

    info!("Requesting {:?} prompt #{}", kind, id);
    // The dialog lives in the main window, which may be hidden in the tray. A recreated window
    // would not be listening yet, so only an existing one is shown
    if app.get_webview_window("main").is_some() {
        let _ = show_main_window(app);
    }

    // Typed binding: the TypeScript generator reads the payload type from it
    let payload: PromptRequest = PromptRequest {
        id,
        kind,
        prefill,
        detail,
    };
    if let Err(e) = app.emit("prompt_requested", payload) {
        error!("Error emitting prompt #{}: {}", id, e);
        prompts.pending.lock().unwrap().remove(&id);
        return None;
    }

    let answer = rx.await.ok().flatten();
    if answer.is_none() {
        warn!("Prompt #{} cancelled by the user", id);
    }
    answer
}

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

/// Asks for the Garmin Connect inputs through the frontend dialogs.
pub struct AppPrompter<R: Runtime>(pub AppHandle<R>);

impl<R: Runtime> ConnectPrompter for AppPrompter<R> {
    fn credentials(
        &self,
        email: Option<String>,
        error: Option<String>,
    ) -> BoxFuture<'static, Option<(String, String)>> {
        let app = self.0.clone();
        async move {
            let mut values = request_prompt(&app, PromptKind::ConnectCredentials, email, error)
                .await?
                .into_iter();
            Some((values.next()?, values.next()?))
        }
        .boxed()
    }

    fn mfa_code(&self, method: String) -> BoxFuture<'static, Option<String>> {
        let app = self.0.clone();
        async move {
            request_prompt(&app, PromptKind::ConnectMfa, None, Some(method))
                .await?
                .into_iter()
                .next()
                .map(|code| code.trim().to_string())
                .filter(|code| !code.is_empty())
        }
        .boxed()
    }
}
