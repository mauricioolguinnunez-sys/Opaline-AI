use sqlx::Row;
use crate::state::AppState;
use axum::http::StatusCode;
use chrono::{NaiveDateTime, Utc};
use uuid::Uuid;
use serde_json::Value;

#[derive(Clone)]
pub struct PlanLimits {
    pub requests: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
}

#[derive(Clone)]
pub struct Reservation {
    pub id: String,
    pub usage_period_id: String,
    pub cached_response: Option<Value>,
    pub cached: bool,
    pub estimated_input_tokens: i64,
    pub estimated_output_tokens: i64,
    pub estimated_total_tokens: i64,
}

pub async fn resolve_current_subscription(pool: &sqlx::MySqlPool, uid: &str) -> Result<(String, String, NaiveDateTime, NaiveDateTime), StatusCode> {
    let row = sqlx::query("SELECT id, plan_id, plan_version_id, current_period_start, current_period_end, start_date, end_date, status FROM subscriptions WHERE user_id=? AND status IN ('active','grace','cancelled') AND (end_date IS NULL OR end_date >= NOW()) ORDER BY is_current DESC, created_at DESC LIMIT 1")
        .bind(uid)
        .fetch_optional(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = row.ok_or(StatusCode::FORBIDDEN)?;
    let subscription_id: String = row.get("id");
    let plan_id: String = row.get("plan_id");
    let plan_version_id: Option<String> = row.try_get("plan_version_id").ok().flatten();
    let plan_version_id = match plan_version_id {
        Some(v) => v,
        None => {
            sqlx::query_scalar("SELECT id FROM plan_versions WHERE plan_id=? ORDER BY version DESC LIMIT 1")
                .bind(&plan_id)
                .fetch_one(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        }
    };
    let period_start: Option<NaiveDateTime> = row.try_get("current_period_start").ok().flatten();
    let period_end: Option<NaiveDateTime> = row.try_get("current_period_end").ok().flatten();
    let start_date: Option<NaiveDateTime> = row.try_get("start_date").ok().flatten();
    let end_date: Option<NaiveDateTime> = row.try_get("end_date").ok().flatten();
    let period_start = period_start.or(start_date).unwrap_or_else(|| Utc::now().naive_utc());
    let period_end = period_end.or(end_date).unwrap_or_else(|| Utc::now().naive_utc() + chrono::Duration::days(30));
    Ok((subscription_id, plan_version_id, period_start, period_end))
}

pub async fn get_plan_limits(pool: &sqlx::MySqlPool, plan_version_id: &str) -> Result<PlanLimits, StatusCode> {
    let row = sqlx::query("SELECT messages_per_month, requests_per_period, input_tokens_per_period, output_tokens_per_period, total_tokens_per_period FROM plan_versions WHERE id=?")
        .bind(plan_version_id)
        .fetch_optional(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let messages: i32 = row.get("messages_per_month");
    let requests: Option<i32> = row.try_get("requests_per_period").ok().flatten();
    let input: Option<i32> = row.try_get("input_tokens_per_period").ok().flatten();
    let output: Option<i32> = row.try_get("output_tokens_per_period").ok().flatten();
    let total: Option<i32> = row.try_get("total_tokens_per_period").ok().flatten();
    Ok(PlanLimits {
        requests: Some(requests.unwrap_or(messages) as i64),
        input_tokens: input.map(|v| v as i64),
        output_tokens: output.map(|v| v as i64),
        total_tokens: total.map(|v| v as i64),
    })
}

async fn ensure_usage_period(tx: &mut sqlx::Transaction<'_, sqlx::MySql>, uid: &str, subscription_id: &str, plan_version_id: &str, period_start: NaiveDateTime, period_end: NaiveDateTime, limits: &PlanLimits) -> Result<String, StatusCode> {
    let row = sqlx::query("SELECT id FROM usage_periods WHERE user_id=? AND period_start=? AND period_end=?")
        .bind(uid).bind(period_start).bind(period_end)
        .fetch_optional(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some(r) = row {
        return Ok(r.get("id"));
    }
    let id = Uuid::new_v4().to_string();
    let result = sqlx::query("INSERT INTO usage_periods (id,user_id,subscription_id,plan_version_id,period_start,period_end,request_limit,input_token_limit,output_token_limit,total_token_limit) VALUES (?,?,?,?,?,?,?,?,?,?)")
        .bind(&id)
        .bind(uid)
        .bind(subscription_id)
        .bind(plan_version_id)
        .bind(period_start)
        .bind(period_end)
        .bind(limits.requests)
        .bind(limits.input_tokens)
        .bind(limits.output_tokens)
        .bind(limits.total_tokens)
        .execute(&mut **tx).await;
    match result {
        Ok(_) => Ok(id),
        Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some("1062") => {
            let r = sqlx::query("SELECT id FROM usage_periods WHERE user_id=? AND period_start=? AND period_end=?")
                .bind(uid).bind(period_start).bind(period_end)
                .fetch_one(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(r.get("id"))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn reserve_quota(
    pool: &sqlx::MySqlPool,
    uid: &str,
    subscription_id: &str,
    plan_version_id: &str,
    period_start: NaiveDateTime,
    period_end: NaiveDateTime,
    limits: &PlanLimits,
    service_type: &str,
    model: &str,
    request_id: &str,
    idempotency_key: &str,
    estimated_input_tokens: i64,
    estimated_output_tokens: i64,
) -> Result<Reservation, StatusCode> {
    let mut tx = pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Idempotency check.
    let existing = sqlx::query("SELECT id, usage_period_id, status, response_json, estimated_input_tokens, estimated_output_tokens, estimated_total_tokens, service_type, model FROM usage_reservations WHERE user_id=? AND idempotency_key=?")
        .bind(uid).bind(idempotency_key)
        .fetch_optional(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some(r) = existing {
        let existing_service: String = r.get("service_type");
        let existing_model: String = r.get("model");
        if existing_service != service_type || existing_model != model {
            tx.rollback().await.ok();
            return Err(StatusCode::CONFLICT);
        }
        let status: String = r.get("status");
        let cached_response: Option<Value> = r.try_get("response_json").ok().flatten();
        if status == "committed" {
            if let Some(resp) = cached_response {
                tx.commit().await.ok();
                return Ok(Reservation {
                    id: r.get("id"),
                    usage_period_id: r.get("usage_period_id"),
                    cached_response: Some(resp),
                    cached: true,
                    estimated_input_tokens: r.get::<i32,_>("estimated_input_tokens") as i64,
                    estimated_output_tokens: r.get::<i32,_>("estimated_output_tokens") as i64,
                    estimated_total_tokens: r.get::<i32,_>("estimated_total_tokens") as i64,
                });
            }
            tx.rollback().await.ok();
            return Err(StatusCode::CONFLICT);
        }

        if matches!(status.as_str(), "released" | "failed" | "expired") {
            let reservation_id: String = r.get("id");
            let period_id: String = r.get("usage_period_id");
            let expires_at = Utc::now().naive_utc() + chrono::Duration::seconds(300);
            sqlx::query("UPDATE usage_reservations SET status='reserved', estimated_input_tokens=?, estimated_output_tokens=?, estimated_total_tokens=?, expires_at=?, recovery_owner=NULL, recovery_lease_expires_at=NULL WHERE id=?")
                .bind(estimated_input_tokens as i32)
                .bind(estimated_output_tokens as i32)
                .bind((estimated_input_tokens + estimated_output_tokens) as i32)
                .bind(expires_at)
                .bind(&reservation_id)
                .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            sqlx::query("UPDATE usage_periods SET reserved_requests=reserved_requests+1, reserved_input_tokens=reserved_input_tokens+?, reserved_output_tokens=reserved_output_tokens+?, reserved_total_tokens=reserved_total_tokens+? WHERE id=?")
                .bind(estimated_input_tokens as i32)
                .bind(estimated_output_tokens as i32)
                .bind((estimated_input_tokens + estimated_output_tokens) as i32)
                .bind(&period_id)
                .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            return Ok(Reservation {
                id: reservation_id,
                usage_period_id: period_id,
                cached_response: None,
                cached: false,
                estimated_input_tokens,
                estimated_output_tokens,
                estimated_total_tokens: estimated_input_tokens + estimated_output_tokens,
            });
        }

        // Reserved or persisting: conflict to avoid duplicate execution.
        tx.rollback().await.ok();
        return Err(StatusCode::CONFLICT);
    }

    let period_id = ensure_usage_period(&mut tx, uid, subscription_id, plan_version_id, period_start, period_end, limits).await?;

    // Lock period.
    let prow = sqlx::query("SELECT request_limit, input_token_limit, output_token_limit, total_token_limit, requests_used, input_tokens_used, output_tokens_used, total_tokens_used, reserved_requests, reserved_input_tokens, reserved_output_tokens, reserved_total_tokens FROM usage_periods WHERE id=? FOR UPDATE")
        .bind(&period_id)
        .fetch_one(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let req_limit: Option<i32> = prow.try_get("request_limit").ok().flatten();
    let in_limit: Option<i32> = prow.try_get("input_token_limit").ok().flatten();
    let out_limit: Option<i32> = prow.try_get("output_token_limit").ok().flatten();
    let total_limit: Option<i32> = prow.try_get("total_token_limit").ok().flatten();
    let req_used: i32 = prow.get("requests_used");
    let in_used: i32 = prow.get("input_tokens_used");
    let out_used: i32 = prow.get("output_tokens_used");
    let total_used: i32 = prow.get("total_tokens_used");
    let req_reserved: i32 = prow.get("reserved_requests");
    let in_reserved: i32 = prow.get("reserved_input_tokens");
    let out_reserved: i32 = prow.get("reserved_output_tokens");
    let total_reserved: i32 = prow.get("reserved_total_tokens");

    let estimated_total = estimated_input_tokens + estimated_output_tokens;
    if let Some(limit) = req_limit {
        if (req_used as i64 + req_reserved as i64 + 1) > limit as i64 {
            tx.rollback().await.ok();
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }
    if let Some(limit) = in_limit {
        if (in_used as i64 + in_reserved as i64 + estimated_input_tokens) > limit as i64 {
            tx.rollback().await.ok();
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }
    if let Some(limit) = out_limit {
        if (out_used as i64 + out_reserved as i64 + estimated_output_tokens) > limit as i64 {
            tx.rollback().await.ok();
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }
    if let Some(limit) = total_limit {
        if (total_used as i64 + total_reserved as i64 + estimated_total) > limit as i64 {
            tx.rollback().await.ok();
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }

    sqlx::query("UPDATE usage_periods SET reserved_requests=reserved_requests+1, reserved_input_tokens=reserved_input_tokens+?, reserved_output_tokens=reserved_output_tokens+?, reserved_total_tokens=reserved_total_tokens+? WHERE id=?")
        .bind(estimated_input_tokens as i32)
        .bind(estimated_output_tokens as i32)
        .bind(estimated_total as i32)
        .bind(&period_id)
        .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let reservation_id = Uuid::new_v4().to_string();
    let expires_at = Utc::now().naive_utc() + chrono::Duration::seconds(300);
    let insert = sqlx::query("INSERT INTO usage_reservations (id,user_id,subscription_id,usage_period_id,request_id,idempotency_key,service_type,model,status,estimated_input_tokens,estimated_output_tokens,estimated_total_tokens,expires_at) VALUES (?,?,?,?,?,?,?,?, 'reserved',?,?,?,?)")
        .bind(&reservation_id)
        .bind(uid)
        .bind(subscription_id)
        .bind(&period_id)
        .bind(request_id)
        .bind(idempotency_key)
        .bind(service_type)
        .bind(model)
        .bind(estimated_input_tokens as i32)
        .bind(estimated_output_tokens as i32)
        .bind(estimated_total as i32)
        .bind(expires_at)
        .execute(&mut *tx).await;
    match insert {
        Ok(_) => {}
        Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some("1062") => {
            tx.rollback().await.ok();
            return Err(StatusCode::CONFLICT);
        }
        Err(_) => {
            tx.rollback().await.ok();
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Reservation {
        id: reservation_id,
        usage_period_id: period_id,
        cached_response: None,
        cached: false,
        estimated_input_tokens,
        estimated_output_tokens,
        estimated_total_tokens: estimated_total,
    })
}

pub async fn commit_reservation(
    pool: &sqlx::MySqlPool,
    reservation_id: &str,
    actual_input_tokens: i64,
    actual_output_tokens: i64,
    token_source: &str,
    measurement_status: &str,
    duration_ms: i64,
    response_json: Value,
) -> Result<(), StatusCode> {
    let mut tx = pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = sqlx::query("SELECT usage_period_id, estimated_input_tokens, estimated_output_tokens, estimated_total_tokens, status, request_id, user_id, subscription_id, model, service_type FROM usage_reservations WHERE id=? FOR UPDATE")
        .bind(reservation_id)
        .fetch_optional(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = match row {
        Some(r) => r,
        None => {
            tx.rollback().await.ok();
            return Err(StatusCode::NOT_FOUND);
        }
    };
    let status: String = row.get("status");
    if status == "committed" {
        tx.commit().await.ok();
        return Ok(());
    }
    if status != "reserved" {
        tx.rollback().await.ok();
        return Err(StatusCode::CONFLICT);
    }
    let period_id: String = row.get("usage_period_id");
    let est_in: i32 = row.get("estimated_input_tokens");
    let est_out: i32 = row.get("estimated_output_tokens");
    let est_total: i32 = row.get("estimated_total_tokens");
    let request_id: String = row.get("request_id");
    let uid: String = row.get("user_id");
    let subscription_id: String = row.get("subscription_id");
    let model: String = row.get("model");
    let service_type: String = row.get("service_type");
    let actual_total = actual_input_tokens + actual_output_tokens;

    sqlx::query("UPDATE usage_reservations SET status='committed', actual_input_tokens=?, actual_output_tokens=?, actual_total_tokens=?, token_source=?, measurement_status=?, committed_at=NOW(), response_json=? WHERE id=?")
        .bind(actual_input_tokens as i32)
        .bind(actual_output_tokens as i32)
        .bind(actual_total as i32)
        .bind(token_source)
        .bind(measurement_status)
        .bind(response_json.to_string())
        .bind(reservation_id)
        .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    sqlx::query("UPDATE usage_periods SET reserved_requests=GREATEST(0,reserved_requests-1), reserved_input_tokens=GREATEST(0,reserved_input_tokens-?), reserved_output_tokens=GREATEST(0,reserved_output_tokens-?), reserved_total_tokens=GREATEST(0,reserved_total_tokens-?), requests_used=requests_used+1, input_tokens_used=input_tokens_used+?, output_tokens_used=output_tokens_used+?, total_tokens_used=total_tokens_used+? WHERE id=?")
        .bind(est_in)
        .bind(est_out)
        .bind(est_total)
        .bind(actual_input_tokens as i32)
        .bind(actual_output_tokens as i32)
        .bind(actual_total as i32)
        .bind(&period_id)
        .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let record_id = Uuid::new_v4().to_string();
    let insert = sqlx::query("INSERT INTO usage_records (id,user_id,subscription_id,usage_period_id,reservation_id,request_id,provider,model,service_type,input_tokens,output_tokens,total_tokens,token_source,measurement_status,duration_ms,status) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?, 'committed')")
        .bind(&record_id)
        .bind(&uid)
        .bind(&subscription_id)
        .bind(&period_id)
        .bind(reservation_id)
        .bind(&request_id)
        .bind("lmstudio")
        .bind(&model)
        .bind(&service_type)
        .bind(actual_input_tokens as i32)
        .bind(actual_output_tokens as i32)
        .bind(actual_total as i32)
        .bind(token_source)
        .bind(measurement_status)
        .bind(duration_ms as i32)
        .execute(&mut *tx).await;
    match insert {
        Ok(_) => {}
        Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some("1062") => {}
        Err(_) => {
            tx.rollback().await.ok();
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

pub async fn release_reservation(pool: &sqlx::MySqlPool, reservation_id: &str, failed: bool) -> Result<(), StatusCode> {
    let mut tx = pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = sqlx::query("SELECT usage_period_id, estimated_input_tokens, estimated_output_tokens, estimated_total_tokens, status FROM usage_reservations WHERE id=? FOR UPDATE")
        .bind(reservation_id)
        .fetch_optional(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = match row {
        Some(r) => r,
        None => {
            tx.rollback().await.ok();
            return Err(StatusCode::NOT_FOUND);
        }
    };
    let status: String = row.get("status");
    if status != "reserved" {
        tx.commit().await.ok();
        return Ok(());
    }
    let period_id: String = row.get("usage_period_id");
    let est_in: i32 = row.get("estimated_input_tokens");
    let est_out: i32 = row.get("estimated_output_tokens");
    let est_total: i32 = row.get("estimated_total_tokens");
    let new_status = if failed { "failed" } else { "released" };
    sqlx::query("UPDATE usage_reservations SET status=?, released_at=NOW() WHERE id=?")
        .bind(new_status)
        .bind(reservation_id)
        .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("UPDATE usage_periods SET reserved_requests=GREATEST(0,reserved_requests-1), reserved_input_tokens=GREATEST(0,reserved_input_tokens-?), reserved_output_tokens=GREATEST(0,reserved_output_tokens-?), reserved_total_tokens=GREATEST(0,reserved_total_tokens-?) WHERE id=?")
        .bind(est_in)
        .bind(est_out)
        .bind(est_total)
        .bind(&period_id)
        .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

pub async fn reap_expired_reservations(pool: &sqlx::MySqlPool) -> Result<u64, StatusCode> {
    let rows = sqlx::query("SELECT id FROM usage_reservations WHERE status='reserved' AND expires_at < NOW()")
        .fetch_all(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut count = 0u64;
    for r in rows {
        let id: String = r.get("id");
        let owner = Uuid::new_v4().to_string();
        let lease_until = Utc::now().naive_utc() + chrono::Duration::seconds(60);
        let claim = sqlx::query("UPDATE usage_reservations SET recovery_owner=?, recovery_lease_expires_at=? WHERE id=? AND status='reserved' AND expires_at < NOW() AND (recovery_owner IS NULL OR recovery_lease_expires_at < NOW() OR recovery_lease_expires_at IS NULL)")
            .bind(&owner)
            .bind(lease_until)
            .bind(&id)
            .execute(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if claim.rows_affected() == 0 {
            continue;
        }
        if release_reservation(pool, &id, false).await.is_ok() {
            let _ = sqlx::query("UPDATE usage_reservations SET status='expired', recovery_owner=NULL, recovery_lease_expires_at=NULL WHERE id=? AND status='released'")
                .bind(&id).execute(pool).await;
            count += 1;
        } else {
            let _ = sqlx::query("UPDATE usage_reservations SET recovery_owner=NULL, recovery_lease_expires_at=NULL WHERE id=?")
                .bind(&id).execute(pool).await;
        }
    }
    Ok(count)
}

pub async fn reconcile_usage(pool: &sqlx::MySqlPool, apply: bool) -> Result<serde_json::Value, StatusCode> {
    let periods = sqlx::query("SELECT id, user_id, period_start, period_end, request_limit, input_token_limit, output_token_limit, total_token_limit, requests_used, input_tokens_used, output_tokens_used, total_tokens_used, reserved_requests, reserved_input_tokens, reserved_output_tokens, reserved_total_tokens FROM usage_periods")
        .fetch_all(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut report = Vec::new();
    for p in periods {
        let period_id: String = p.get("id");
        let actual_row = sqlx::query("SELECT COUNT(*) AS requests, COALESCE(SUM(input_tokens),0) AS input_tokens, COALESCE(SUM(output_tokens),0) AS output_tokens, COALESCE(SUM(total_tokens),0) AS total_tokens FROM usage_records WHERE usage_period_id=?")
            .bind(&period_id)
            .fetch_one(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let reserved_row = sqlx::query("SELECT COALESCE(SUM(estimated_input_tokens),0) AS input_tokens, COALESCE(SUM(estimated_output_tokens),0) AS output_tokens, COALESCE(SUM(estimated_total_tokens),0) AS total_tokens, COUNT(*) AS requests FROM usage_reservations WHERE usage_period_id=? AND status='reserved'")
            .bind(&period_id)
            .fetch_one(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let actual_requests: i64 = actual_row.get("requests");
        let actual_input: i64 = actual_row.get("input_tokens");
        let actual_output: i64 = actual_row.get("output_tokens");
        let actual_total: i64 = actual_row.get("total_tokens");
        let reserved_requests: i64 = reserved_row.get("requests");
        let reserved_input: i64 = reserved_row.get("input_tokens");
        let reserved_output: i64 = reserved_row.get("output_tokens");
        let reserved_total: i64 = reserved_row.get("total_tokens");

        let requests_used: i32 = p.get("requests_used");
        let input_used: i32 = p.get("input_tokens_used");
        let output_used: i32 = p.get("output_tokens_used");
        let total_used: i32 = p.get("total_tokens_used");
        let reserved_requests_db: i32 = p.get("reserved_requests");
        let reserved_input_db: i32 = p.get("reserved_input_tokens");
        let reserved_output_db: i32 = p.get("reserved_output_tokens");
        let reserved_total_db: i32 = p.get("reserved_total_tokens");

        let mismatch = requests_used as i64 != actual_requests
            || input_used as i64 != actual_input
            || output_used as i64 != actual_output
            || total_used as i64 != actual_total
            || reserved_requests_db as i64 != reserved_requests
            || reserved_input_db as i64 != reserved_input
            || reserved_output_db as i64 != reserved_output
            || reserved_total_db as i64 != reserved_total;

        if apply && mismatch {
            sqlx::query("UPDATE usage_periods SET requests_used=?, input_tokens_used=?, output_tokens_used=?, total_tokens_used=?, reserved_requests=?, reserved_input_tokens=?, reserved_output_tokens=?, reserved_total_tokens=? WHERE id=?")
                .bind(actual_requests as i32)
                .bind(actual_input as i32)
                .bind(actual_output as i32)
                .bind(actual_total as i32)
                .bind(reserved_requests as i32)
                .bind(reserved_input as i32)
                .bind(reserved_output as i32)
                .bind(reserved_total as i32)
                .bind(&period_id)
                .execute(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }

        // Committed reservations without usage records.
        let missing_records: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM usage_reservations r LEFT JOIN usage_records rec ON rec.reservation_id=r.id WHERE r.usage_period_id=? AND r.status='committed' AND rec.id IS NULL")
            .bind(&period_id).fetch_one(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        report.push(serde_json::json!({
            "usage_period_id": period_id,
            "mismatch": mismatch,
            "applied": apply && mismatch,
            "actual_requests": actual_requests,
            "actual_input_tokens": actual_input,
            "actual_output_tokens": actual_output,
            "actual_total_tokens": actual_total,
            "reserved_requests": reserved_requests,
            "committed_without_record": missing_records,
        }));
    }
    Ok(serde_json::json!({"periods": report}))
}

pub async fn get_current_usage(pool: &sqlx::MySqlPool, uid: &str) -> Result<Value, StatusCode> {
    let row = sqlx::query("SELECT id, period_start, period_end, request_limit, input_token_limit, output_token_limit, total_token_limit, requests_used, input_tokens_used, output_tokens_used, total_tokens_used, reserved_requests, reserved_input_tokens, reserved_output_tokens, reserved_total_tokens, status FROM usage_periods WHERE user_id=? ORDER BY period_start DESC LIMIT 1")
        .bind(uid)
        .fetch_optional(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    match row {
        Some(r) => {
            let period_start: NaiveDateTime = r.get("period_start");
            let period_end: NaiveDateTime = r.get("period_end");
            Ok(serde_json::json!({
                "period_start": period_start.to_string(),
                "period_end": period_end.to_string(),
                "request_limit": r.try_get::<Option<i32>,_>("request_limit").ok().flatten(),
                "input_token_limit": r.try_get::<Option<i32>,_>("input_token_limit").ok().flatten(),
                "output_token_limit": r.try_get::<Option<i32>,_>("output_token_limit").ok().flatten(),
                "total_token_limit": r.try_get::<Option<i32>,_>("total_token_limit").ok().flatten(),
                "requests_used": r.get::<i32,_>("requests_used"),
                "input_tokens_used": r.get::<i32,_>("input_tokens_used"),
                "output_tokens_used": r.get::<i32,_>("output_tokens_used"),
                "total_tokens_used": r.get::<i32,_>("total_tokens_used"),
                "reserved_requests": r.get::<i32,_>("reserved_requests"),
                "reserved_input_tokens": r.get::<i32,_>("reserved_input_tokens"),
                "reserved_output_tokens": r.get::<i32,_>("reserved_output_tokens"),
                "reserved_total_tokens": r.get::<i32,_>("reserved_total_tokens"),
                "status": r.get::<String,_>("status"),
            }))
        }
        None => Ok(serde_json::json!({
            "period_start": null,
            "period_end": null,
            "request_limit": null,
            "input_token_limit": null,
            "output_token_limit": null,
            "total_token_limit": null,
            "requests_used": 0,
            "input_tokens_used": 0,
            "output_tokens_used": 0,
            "total_tokens_used": 0,
            "reserved_requests": 0,
            "reserved_input_tokens": 0,
            "reserved_output_tokens": 0,
            "reserved_total_tokens": 0,
            "status": "none",
        })),
    }
}

pub async fn get_usage_history(pool: &sqlx::MySqlPool, uid: &str, limit: i64, offset: i64) -> Result<Value, StatusCode> {
    let limit = limit.clamp(1, 100);
    let offset = offset.max(0);
    let rows = sqlx::query("SELECT id, request_id, provider, model, service_type, input_tokens, output_tokens, total_tokens, token_source, measurement_status, duration_ms, status, created_at FROM usage_records WHERE user_id=? ORDER BY created_at DESC LIMIT ? OFFSET ?")
        .bind(uid).bind(limit).bind(offset)
        .fetch_all(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut out = Vec::new();
    for r in rows {
        let created: NaiveDateTime = r.get("created_at");
        out.push(serde_json::json!({
            "id": r.get::<String,_>("id"),
            "request_id": r.get::<String,_>("request_id"),
            "provider": r.get::<String,_>("provider"),
            "model": r.get::<String,_>("model"),
            "service_type": r.get::<String,_>("service_type"),
            "input_tokens": r.get::<i32,_>("input_tokens"),
            "output_tokens": r.get::<i32,_>("output_tokens"),
            "total_tokens": r.get::<i32,_>("total_tokens"),
            "token_source": r.get::<String,_>("token_source"),
            "measurement_status": r.get::<String,_>("measurement_status"),
            "duration_ms": r.get::<i32,_>("duration_ms"),
            "status": r.get::<String,_>("status"),
            "created_at": created.to_string(),
        }));
    }
    Ok(serde_json::json!(out))
}

pub async fn get_usage_summary(pool: &sqlx::MySqlPool, uid: &str) -> Result<Value, StatusCode> {
    let row = sqlx::query("SELECT COUNT(*) AS requests, COALESCE(SUM(input_tokens),0) AS input_tokens, COALESCE(SUM(output_tokens),0) AS output_tokens, COALESCE(SUM(total_tokens),0) AS total_tokens FROM usage_records WHERE user_id=?")
        .bind(uid)
        .fetch_one(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(serde_json::json!({
        "requests": row.get::<i64,_>("requests"),
        "input_tokens": row.get::<i64,_>("input_tokens"),
        "output_tokens": row.get::<i64,_>("output_tokens"),
        "total_tokens": row.get::<i64,_>("total_tokens"),
    }))
}

pub fn estimate_tokens(text: &str) -> i64 {
    let chars = text.chars().count() as f64;
    (chars / 4.0).ceil().max(1.0) as i64
}