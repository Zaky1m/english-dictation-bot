use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    Month,
    Year,
}

impl Plan {
    pub fn name(self) -> &'static str {
        match self {
            Self::Month => "month",
            Self::Year => "year",
        }
    }

    pub fn days(self) -> i64 {
        match self {
            Self::Month => 30,
            Self::Year => 365,
        }
    }
}

pub fn invoice_payload(plan: Plan, user_id: i64) -> String {
    format!("sub:{}:{user_id}", plan.name())
}

/// Parses `sub:<plan>:<user id>`.
pub fn parse_payload(payload: &str) -> Option<(Plan, i64)> {
    let mut parts = payload.split(':');
    let (Some("sub"), Some(plan), Some(user), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    let plan = match plan {
        "month" => Plan::Month,
        "year" => Plan::Year,
        _ => return None,
    };
    Some((plan, user.parse().ok()?))
}

/// A purchase extends the current subscription instead of overlapping it.
pub fn extend(
    now: DateTime<Utc>,
    current_end: Option<DateTime<Utc>>,
    plan: Plan,
) -> (DateTime<Utc>, DateTime<Utc>) {
    extend_days(now, current_end, plan.days())
}

/// Same rule for any length, used by manual grants from the admin panel.
pub fn extend_days(
    now: DateTime<Utc>,
    current_end: Option<DateTime<Utc>>,
    days: i64,
) -> (DateTime<Utc>, DateTime<Utc>) {
    let starts = current_end.filter(|end| *end > now).unwrap_or(now);
    (starts, starts + Duration::days(days))
}

pub struct Payment<'a> {
    pub user_id: i64,
    pub charge_id: &'a str,
    pub amount: i64,
    pub currency: &'a str,
    pub payload: &'a str,
}

/// Stores a Stars payment and extends Pro, once per charge id. Returns the new
/// end date, or `None` when this charge was already recorded.
pub async fn record(
    pool: &PgPool,
    payment: &Payment<'_>,
    now: DateTime<Utc>,
) -> Result<Option<(Plan, DateTime<Utc>)>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let inserted = sqlx::query(
        "INSERT INTO payments (telegram_payment_charge_id, user_id, amount, currency, payload)
         VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING",
    )
    .bind(payment.charge_id)
    .bind(payment.user_id)
    .bind(payment.amount as i32)
    .bind(payment.currency)
    .bind(payment.payload)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if inserted == 0 {
        return Ok(None);
    }
    let Some((plan, payload_user)) = parse_payload(payment.payload) else {
        tracing::error!(
            charge = payment.charge_id,
            "payment with unknown payload recorded without Pro"
        );
        tx.commit().await?;
        return Ok(None);
    };
    if payload_user != payment.user_id {
        tracing::error!(charge = payment.charge_id, "payment payload user mismatch");
        tx.commit().await?;
        return Ok(None);
    }
    let current_end: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT max(ends_at) FROM subscriptions WHERE user_id = $1")
            .bind(payment.user_id)
            .fetch_one(&mut *tx)
            .await?;
    let (starts, ends) = extend(now, current_end, plan);
    sqlx::query(
        "INSERT INTO subscriptions (user_id, plan, source, starts_at, ends_at, stars_charge_id)
         VALUES ($1, $2, 'stars', $3, $4, $5)",
    )
    .bind(payment.user_id)
    .bind(plan.name())
    .bind(starts)
    .bind(ends)
    .bind(payment.charge_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some((plan, ends)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, day, 12, 0, 0).unwrap()
    }

    #[test]
    fn payload_round_trips_and_rejects_garbage() {
        let payload = invoice_payload(Plan::Year, 42);
        assert_eq!(payload, "sub:year:42");
        assert_eq!(parse_payload(&payload), Some((Plan::Year, 42)));
        for bad in [
            "",
            "sub:week:1",
            "sub:month",
            "sub:month:x",
            "pay:month:1",
            "sub:month:1:2",
        ] {
            assert_eq!(parse_payload(bad), None, "{bad}");
        }
    }

    #[test]
    fn new_purchase_starts_now() {
        let (starts, ends) = extend(at(1), None, Plan::Month);
        assert_eq!(starts, at(1));
        assert_eq!(ends, at(31));
    }

    #[test]
    fn active_subscription_is_extended_not_overlapped() {
        let (starts, ends) = extend(at(1), Some(at(10)), Plan::Month);
        assert_eq!(starts, at(10));
        assert_eq!(ends, at(10) + Duration::days(30));
    }

    #[test]
    fn expired_subscription_restarts_from_now() {
        let (starts, _) = extend(at(20), Some(at(10)), Plan::Year);
        assert_eq!(starts, at(20));
    }
}
