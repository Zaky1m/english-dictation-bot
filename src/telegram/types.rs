use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Update {
    pub update_id: i64,
    pub message: Option<Message>,
    pub callback_query: Option<CallbackQuery>,
    pub pre_checkout_query: Option<PreCheckoutQuery>,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub chat: Chat,
    pub from: Option<Sender>,
    pub text: Option<String>,
    pub successful_payment: Option<SuccessfulPayment>,
}

#[derive(Debug, Deserialize)]
pub struct Chat {
    pub id: i64,
}

#[derive(Debug, Deserialize)]
pub struct Sender {
    pub id: i64,
    #[serde(default)]
    pub first_name: String,
    #[serde(default)]
    pub last_name: String,
    pub username: Option<String>,
}

impl Sender {
    pub fn display_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
            .trim()
            .to_owned()
    }
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub id: String,
    pub from: Sender,
    pub data: Option<String>,
    pub message: Option<MessageRef>,
}

#[derive(Debug, Deserialize)]
pub struct MessageRef {
    pub message_id: i64,
    pub chat: Chat,
}

#[derive(Debug, Deserialize)]
pub struct PreCheckoutQuery {
    pub id: String,
    pub from: Sender,
    pub currency: String,
    pub total_amount: i64,
    pub invoice_payload: String,
}

#[derive(Debug, Deserialize)]
pub struct SuccessfulPayment {
    pub currency: String,
    pub total_amount: i64,
    pub invoice_payload: String,
    pub telegram_payment_charge_id: String,
}
