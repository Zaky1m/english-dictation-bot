use super::AppState;
use super::error::{ApiError, ApiResult};
use super::user::User;
use crate::telegram::handlers::{payload_for, stars_price};
use crate::telegram::payments::Plan;
use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};

/// Creates a Stars invoice link for the Mini App to open. The monthly plan is a
/// recurring Telegram subscription; the yearly plan is a one-time payment.
pub async fn stars_invoice(
    State(state): State<AppState>,
    user: User,
    Path(plan): Path<String>,
) -> ApiResult<Value> {
    let (plan, title_key) = match plan.as_str() {
        "month" => (Plan::Month, "invoice_title_month"),
        "year" => (Plan::Year, "invoice_title_year"),
        _ => return Err(ApiError::NotFound),
    };
    let period = (plan == Plan::Month).then(|| state.settings.int("stars_month_period_secs"));
    let url = state
        .bot
        .create_invoice_link(
            &state.texts.render(&user.lang, title_key, &[]),
            &state.texts.render(&user.lang, "invoice_description", &[]),
            &payload_for(plan, user.id),
            stars_price(&state, plan),
            period,
        )
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;
    Ok(Json(json!({ "url": url })))
}
