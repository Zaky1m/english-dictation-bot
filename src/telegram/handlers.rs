use super::bot::{BotError, callback_button, keyboard, web_app_button};
use super::payments::{self, Payment, Plan, invoice_payload};
use super::types::{CallbackQuery, Message, PreCheckoutQuery, Update};
use crate::http::AppState;
use crate::users;
use chrono::Utc;

/// Handles one update inline. Errors bubble up so the webhook answers 500 and
/// Telegram retries; a blocked user is not an error.
pub async fn handle(state: &AppState, update: Update) -> anyhow::Result<()> {
    let result = if let Some(message) = update.message {
        on_message(state, message).await
    } else if let Some(query) = update.callback_query {
        on_callback(state, query).await
    } else if let Some(query) = update.pre_checkout_query {
        on_pre_checkout(state, query).await
    } else {
        Ok(())
    };
    match result {
        Err(HandlerError::Bot(BotError::Blocked)) => Ok(()),
        Err(e) => Err(e.into()),
        Ok(()) => Ok(()),
    }
}

#[derive(Debug, thiserror::Error)]
enum HandlerError {
    #[error(transparent)]
    Bot(#[from] BotError),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

async fn on_message(state: &AppState, message: Message) -> Result<(), HandlerError> {
    let Some(sender) = &message.from else {
        return Ok(());
    };
    let profile = users::upsert(
        &state.pool,
        sender.id,
        &sender.display_name(),
        sender.username.as_deref(),
    )
    .await?;
    if profile.blocked {
        return Ok(());
    }
    // Writing to the bot proves the chat is open again.
    sqlx::query("UPDATE users SET bot_blocked = false WHERE id = $1 AND bot_blocked")
        .bind(sender.id)
        .execute(&state.pool)
        .await?;
    if let Some(payment) = &message.successful_payment {
        return on_payment(state, sender.id, message.chat.id, &profile.lang, payment).await;
    }
    let is_start = message
        .text
        .as_deref()
        .is_some_and(|t| t == "/start" || t.starts_with("/start "));
    if is_start {
        send_welcome(
            state,
            message.chat.id,
            sender.id,
            &sender.display_name(),
            &profile.lang,
        )
        .await
    } else {
        let text = state.texts.render(&profile.lang, "unknown", &[]);
        Ok(state.bot.send_message(message.chat.id, &text, None).await?)
    }
}

fn welcome_keyboard(state: &AppState, user_id: i64, lang: &str) -> serde_json::Value {
    let t = &state.texts;
    let url = &state.config.public_url;
    let mut rows = vec![vec![web_app_button(
        &t.render(lang, "open_app", &[]),
        url,
        "primary",
    )]];
    // One button per other language, labelled in that language, three to a row.
    let others: Vec<_> = crate::languages::all()
        .iter()
        .filter(|l| l.code != lang)
        .map(|l| callback_button(&l.name, &format!("lang:{}", l.code), "success"))
        .collect();
    for row in others.chunks(3) {
        rows.push(row.to_vec());
    }
    if state.config.admin_ids.contains(&user_id) {
        rows.push(vec![web_app_button(
            &t.render(lang, "open_admin", &[]),
            &format!("{url}/admin"),
            "danger",
        )]);
    }
    keyboard(rows)
}

async fn send_welcome(
    state: &AppState,
    chat_id: i64,
    user_id: i64,
    name: &str,
    lang: &str,
) -> Result<(), HandlerError> {
    let text = state.texts.render(lang, "welcome", &[("name", name)]);
    Ok(state
        .bot
        .send_message(chat_id, &text, Some(welcome_keyboard(state, user_id, lang)))
        .await?)
}

async fn on_callback(state: &AppState, query: CallbackQuery) -> Result<(), HandlerError> {
    let sender = &query.from;
    let profile = users::upsert(
        &state.pool,
        sender.id,
        &sender.display_name(),
        sender.username.as_deref(),
    )
    .await?;
    state.bot.answer_callback(&query.id).await?;
    let (Some(data), Some(message)) = (query.data.as_deref(), &query.message) else {
        return Ok(());
    };
    let chosen_lang = data
        .strip_prefix("lang:")
        .filter(|code| crate::languages::is_supported(code));
    match data {
        _ if chosen_lang.is_some() => {
            let lang = chosen_lang.unwrap_or_default();
            sqlx::query("UPDATE users SET lang = $2 WHERE id = $1")
                .bind(sender.id)
                .bind(lang)
                .execute(&state.pool)
                .await?;
            let text = state.texts.render(lang, "lang_changed", &[]);
            state
                .bot
                .edit_message(
                    message.chat.id,
                    message.message_id,
                    &text,
                    Some(welcome_keyboard(state, sender.id, lang)),
                )
                .await?;
        }
        "rem:off" => {
            sqlx::query("UPDATE users SET reminders_on = false WHERE id = $1")
                .bind(sender.id)
                .execute(&state.pool)
                .await?;
            let text = state.texts.render(&profile.lang, "reminders_off", &[]);
            state
                .bot
                .edit_message(message.chat.id, message.message_id, &text, None)
                .await?;
        }
        _ => {}
    }
    Ok(())
}

async fn on_pre_checkout(state: &AppState, query: PreCheckoutQuery) -> Result<(), HandlerError> {
    let lang: Option<String> = sqlx::query_scalar("SELECT lang FROM users WHERE id = $1")
        .bind(query.from.id)
        .fetch_optional(&state.pool)
        .await?;
    let lang = lang.unwrap_or_else(|| crate::languages::default_code().into());
    let expected = match payments::parse_payload(&query.invoice_payload) {
        Some((plan, user)) if user == query.from.id => Some(stars_price(state, plan)),
        _ => None,
    };
    let valid = query.currency == "XTR" && expected == Some(query.total_amount);
    let error = (!valid).then(|| state.texts.render(&lang, "pay_rejected", &[]));
    Ok(state
        .bot
        .answer_pre_checkout(&query.id, error.as_deref())
        .await?)
}

pub fn stars_price(state: &AppState, plan: Plan) -> i64 {
    match plan {
        Plan::Month => state.settings.int("stars_month"),
        Plan::Year => state.settings.int("stars_year"),
    }
}

async fn on_payment(
    state: &AppState,
    user_id: i64,
    chat_id: i64,
    lang: &str,
    payment: &super::types::SuccessfulPayment,
) -> Result<(), HandlerError> {
    let recorded = payments::record(
        &state.pool,
        &Payment {
            user_id,
            charge_id: &payment.telegram_payment_charge_id,
            amount: payment.total_amount,
            currency: &payment.currency,
            payload: &payment.invoice_payload,
        },
        Utc::now(),
    )
    .await?;
    if let Some((plan, ends)) = recorded {
        let plan_text = state.texts.render(
            lang,
            if plan == Plan::Year {
                "plan_year"
            } else {
                "plan_month"
            },
            &[],
        );
        let until = ends.format("%Y-%m-%d").to_string();
        let text = state.texts.render(
            lang,
            "payment_thanks",
            &[("plan", &plan_text), ("until", &until)],
        );
        state.bot.send_message(chat_id, &text, None).await?;
    }
    Ok(())
}

/// Used by the invoice endpoint so the payload format lives in one place.
pub fn payload_for(plan: Plan, user_id: i64) -> String {
    invoice_payload(plan, user_id)
}
