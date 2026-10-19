use serde::Serialize;
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum BotError {
    /// The user blocked the bot or deleted the chat (HTTP 403).
    #[error("user blocked the bot")]
    Blocked,
    #[error("telegram error {code}: {description}")]
    Api { code: i64, description: String },
    #[error("telegram transport error: {0}")]
    Transport(String),
}

/// Thin Bot API client. The token lives only inside `base`; it is never logged.
pub struct Bot {
    http: reqwest::Client,
    base: String,
}

impl Bot {
    pub fn new(token: &str, api_origin: &str) -> anyhow::Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()?,
            base: format!("{}/bot{token}", api_origin.trim_end_matches('/')),
        })
    }

    pub async fn call(&self, method: &str, body: &impl Serialize) -> Result<Value, BotError> {
        let response = self
            .http
            .post(format!("{}/{method}", self.base))
            .json(body)
            .send()
            .await
            // The error text would include the URL, and the URL holds the token.
            .map_err(|e| BotError::Transport(e.without_url().to_string()))?;
        let status = response.status();
        let value: Value = response
            .json()
            .await
            .map_err(|e| BotError::Transport(e.without_url().to_string()))?;
        if value["ok"].as_bool() == Some(true) {
            return Ok(value["result"].clone());
        }
        let code = value["error_code"]
            .as_i64()
            .unwrap_or(i64::from(status.as_u16()));
        if code == 403 {
            return Err(BotError::Blocked);
        }
        Err(BotError::Api {
            code,
            description: value["description"].as_str().unwrap_or("").to_owned(),
        })
    }

    pub async fn send_message(
        &self,
        chat_id: i64,
        text: &str,
        keyboard: Option<Value>,
    ) -> Result<(), BotError> {
        let mut body = json!({ "chat_id": chat_id, "text": text, "parse_mode": "HTML" });
        if let Some(kb) = keyboard {
            body["reply_markup"] = kb;
        }
        self.call("sendMessage", &body).await.map(drop)
    }

    pub async fn edit_message(
        &self,
        chat_id: i64,
        message_id: i64,
        text: &str,
        keyboard: Option<Value>,
    ) -> Result<(), BotError> {
        let mut body = json!({ "chat_id": chat_id, "message_id": message_id, "text": text, "parse_mode": "HTML" });
        if let Some(kb) = keyboard {
            body["reply_markup"] = kb;
        }
        self.call("editMessageText", &body).await.map(drop)
    }

    pub async fn answer_callback(&self, id: &str) -> Result<(), BotError> {
        self.call("answerCallbackQuery", &json!({ "callback_query_id": id }))
            .await
            .map(drop)
    }

    pub async fn answer_pre_checkout(&self, id: &str, error: Option<&str>) -> Result<(), BotError> {
        let body = match error {
            None => json!({ "pre_checkout_query_id": id, "ok": true }),
            Some(message) => {
                json!({ "pre_checkout_query_id": id, "ok": false, "error_message": message })
            }
        };
        self.call("answerPreCheckoutQuery", &body).await.map(drop)
    }

    pub async fn set_webhook(&self, url: &str, secret: &str) -> Result<(), BotError> {
        self.call(
            "setWebhook",
            &json!({
                "url": url,
                "secret_token": secret,
                "allowed_updates": ["message", "callback_query", "pre_checkout_query"],
            }),
        )
        .await
        .map(drop)
    }

    pub async fn set_menu_button(&self, text: &str, url: &str) -> Result<(), BotError> {
        self.call(
            "setChatMenuButton",
            &json!({ "menu_button": { "type": "web_app", "text": text, "web_app": { "url": url } } }),
        )
        .await
        .map(drop)
    }

    pub async fn create_invoice_link(
        &self,
        title: &str,
        description: &str,
        payload: &str,
        stars: i64,
        subscription_period_secs: Option<i64>,
    ) -> Result<String, BotError> {
        let mut body = json!({
            "title": title,
            "description": description,
            "payload": payload,
            "currency": "XTR",
            "prices": [{ "label": title, "amount": stars }],
        });
        if let Some(period) = subscription_period_secs {
            body["subscription_period"] = json!(period);
        }
        let result = self.call("createInvoiceLink", &body).await?;
        result
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| BotError::Transport("invoice link missing".into()))
    }
}

/// Inline button that opens the Mini App. `style` is one of primary, success, danger.
pub fn web_app_button(text: &str, url: &str, style: &str) -> Value {
    json!({ "text": text, "web_app": { "url": url }, "style": style })
}

pub fn callback_button(text: &str, data: &str, style: &str) -> Value {
    json!({ "text": text, "callback_data": data, "style": style })
}

pub fn keyboard(rows: Vec<Vec<Value>>) -> Value {
    json!({ "inline_keyboard": rows })
}
