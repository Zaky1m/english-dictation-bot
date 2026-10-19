use super::AppState;
use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

/// Development mode: until an admin opens the app to everyone, only admins get
/// any answer from the bot or the API.
pub fn is_open_to(state: &AppState, user_id: i64) -> bool {
    state.settings.bool("public_access") || state.config.admin_ids.contains(&user_id)
}

/// Where the Mini App sends people it turns away: the bot's chat.
pub async fn bot_link(State(state): State<AppState>) -> Json<Value> {
    let url = state
        .bot
        .username()
        .await
        .map(|name| format!("https://t.me/{name}"));
    Json(json!({ "url": url }))
}
