//! Admin panel API. Skipped unless `TEST_DATABASE_URL` is set.
use super::bot_tests::start_stub;
use super::engagement_tests::state_with_vars;
use super::flow_tests::{Client, init_data, seed};
use super::*;
use crate::db;
use axum::http::StatusCode;
use serde_json::json;

#[tokio::test]
async fn admin_panel_api() {
    let Some((pool, schema)) = db::test_pool().await else {
        return;
    };
    let (lesson1, _) = seed(&pool).await;
    let (_, origin) = start_stub().await;
    let state = state_with_vars(
        &pool,
        &origin,
        &[("ADMIN_IDS", "900"), ("ADMIN_USERNAME", "boss")],
    )
    .await;
    let app = router(state.clone());
    let admin = Client {
        app: app.clone(),
        auth: init_data(900),
    };
    let learner = Client {
        app: app.clone(),
        auth: init_data(901),
    };

    // Only admins get in.
    let (status, body) = learner.call("GET", "/api/admin/overview", None).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("admin_only"))
    );
    let (status, _) = learner
        .call(
            "POST",
            "/api/admin/users/901/grant",
            Some(json!({"plan": "year"})),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (_, me) = learner.call("GET", "/api/me", None).await;
    assert_eq!(me["admin_username"], "boss");

    let (status, ov) = admin.call("GET", "/api/admin/overview", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ov["users"], 2);

    // Search by id and by name; grant, extend and revoke Pro.
    let (_, found) = admin.call("GET", "/api/admin/users?q=901", None).await;
    assert_eq!(found[0]["id"], 901);
    let (_, found) = admin
        .call("GET", "/api/admin/users?q=%40nobody", None)
        .await;
    assert!(found.as_array().unwrap().is_empty());
    let (status, _) = admin
        .call(
            "POST",
            "/api/admin/users/901/grant",
            Some(json!({"plan": "custom", "days": 0})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = admin
        .call(
            "POST",
            "/api/admin/users/901/grant",
            Some(json!({"plan": "weekly"})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = admin
        .call(
            "POST",
            "/api/admin/users/777/grant",
            Some(json!({"plan": "month"})),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = admin
        .call(
            "POST",
            "/api/admin/users/901/grant",
            Some(json!({"plan": "month"})),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = admin
        .call(
            "POST",
            "/api/admin/users/901/grant",
            Some(json!({"plan": "custom", "days": 10})),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (days, granted_by): (i64, i64) = sqlx::query_as(
        "SELECT (max(ends_at)::date - now()::date)::bigint, max(granted_by) FROM subscriptions WHERE user_id = 901",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (days, granted_by),
        (40, 900),
        "the second grant extends the first"
    );
    let (_, me) = learner.call("GET", "/api/me", None).await;
    assert_eq!(me["pro"], true);
    let (status, _) = admin
        .call("POST", "/api/admin/users/901/revoke", None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, me) = learner.call("GET", "/api/me", None).await;
    assert_eq!(me["pro"], false);

    // Payments and manual grants are listed.
    sqlx::query("INSERT INTO payments (telegram_payment_charge_id, user_id, amount, currency, payload) VALUES ('c1', 901, 150, 'XTR', 'sub:month:901')")
        .execute(&pool)
        .await
        .unwrap();
    let (_, pay) = admin.call("GET", "/api/admin/payments", None).await;
    assert_eq!(pay["payments"][0]["charge_id"], "c1");
    assert_eq!(pay["grants"].as_array().unwrap().len(), 2);
    let (_, ov) = admin.call("GET", "/api/admin/overview", None).await;
    assert_eq!(ov["stars_total"], 150);

    // Settings: type-checked, and effective immediately.
    let (_, all) = admin.call("GET", "/api/admin/settings", None).await;
    assert!(
        all.as_array()
            .unwrap()
            .iter()
            .any(|row| row[0] == "free_daily_sentences")
    );
    let (status, _) = admin
        .call(
            "PUT",
            "/api/admin/settings/free_daily_sentences",
            Some(json!({"value": "ten"})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = admin
        .call(
            "PUT",
            "/api/admin/settings/no_such_key",
            Some(json!({"value": 1})),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = admin
        .call(
            "PUT",
            "/api/admin/settings/free_daily_sentences",
            Some(json!({"value": 3})),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, me) = learner.call("GET", "/api/me", None).await;
    assert_eq!(me["usage"]["limit"], 3);
    let (status, _) = learner
        .call(
            "PUT",
            "/api/admin/settings/free_daily_sentences",
            Some(json!({"value": 99})),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Content: counts, disabling a sentence hides it from lessons.
    let (_, sits) = admin.call("GET", "/api/admin/situations", None).await;
    assert_eq!(sits[0]["sentences"], 3);
    assert_eq!(sits[0]["audio_missing"], 3);
    let sit_id = sits[0]["id"].as_i64().unwrap();
    let (_, list) = admin
        .call(
            "GET",
            &format!("/api/admin/situations/{sit_id}/sentences"),
            None,
        )
        .await;
    let first = list[0]["id"].as_i64().unwrap();
    let (status, _) = admin
        .call(
            "POST",
            &format!("/api/admin/sentences/{first}/active"),
            Some(json!({"active": false})),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, a) = learner
        .call("POST", &format!("/api/lessons/{lesson1}/next"), None)
        .await;
    assert_eq!(a["index"], 2, "the disabled first sentence is skipped");

    // Publishing: an unpublished situation with empty lessons cannot go live.
    sqlx::query("INSERT INTO situations (slug, name_ar, name_en, icon, position, active) VALUES ('draft', 'م', 'Draft', 'chat', 5, false)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO lessons (situation_id, number) SELECT id, 1 FROM situations WHERE slug = 'draft'").execute(&pool).await.unwrap();
    let draft: i64 = sqlx::query_scalar("SELECT id::bigint FROM situations WHERE slug = 'draft'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let (status, body) = admin
        .call(
            "POST",
            &format!("/api/admin/situations/{draft}/publish"),
            Some(json!({"active": true})),
        )
        .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("incomplete"))
    );
    let (status, _) = admin
        .call(
            "POST",
            &format!("/api/admin/situations/{sit_id}/publish"),
            Some(json!({"active": true})),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // The generation tools need Gemini; without keys they say so, and bad input is rejected first.
    let new =
        |slug: &str| json!({"slug": slug, "name_ar": "جديد", "name_en": "New", "icon": "chat"});
    let (status, _) = admin
        .call("POST", "/api/admin/situations", Some(new("Bad Slug")))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, body) = admin
        .call("POST", "/api/admin/situations", Some(new("gym")))
        .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("gemini_unavailable"))
    );
    let (status, _) = admin
        .call(
            "POST",
            &format!("/api/admin/situations/{sit_id}/regenerate-audio"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        crate::content::runner::start(&state, crate::content::runner::Kind::Audio(None)).await,
        Err(crate::content::runner::StartError::NoGemini)
    );
    let (_, jobs) = admin.call("GET", "/api/admin/jobs", None).await;
    assert!(jobs.as_array().unwrap().is_empty());

    db::drop_test_schema(&pool, &schema).await;
}
