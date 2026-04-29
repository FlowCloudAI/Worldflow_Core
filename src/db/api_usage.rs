use crate::error::Result;
use crate::models::{ApiUsageByModel, ApiUsageSummary, CreateApiUsageLog};
use sqlx::SqlitePool;

/// 插入一条 API 用量记录
pub async fn insert_api_usage(pool: &SqlitePool, input: &CreateApiUsageLog) -> Result<()> {
    sqlx::query(
        "INSERT INTO api_usage_log (id, session_id, model, provider, modality, \
         prompt_tokens, completion_tokens, total_tokens) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&input.id)
    .bind(&input.session_id)
    .bind(&input.model)
    .bind(&input.provider)
    .bind(&input.modality)
    .bind(input.prompt_tokens)
    .bind(input.completion_tokens)
    .bind(input.total_tokens)
    .execute(pool)
    .await?;
    Ok(())
}

/// 查询用量总览
pub async fn query_usage_summary(pool: &SqlitePool) -> Result<ApiUsageSummary> {
    let row = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        "SELECT COALESCE(SUM(prompt_tokens), 0), COALESCE(SUM(completion_tokens), 0), \
         COALESCE(SUM(total_tokens), 0), COUNT(*) \
         FROM api_usage_log",
    )
    .fetch_one(pool)
    .await?;

    Ok(ApiUsageSummary {
        total_prompt_tokens: row.0,
        total_completion_tokens: row.1,
        total_tokens: row.2,
        call_count: row.3,
    })
}

/// 按模型分组查询用量
pub async fn query_usage_by_model(pool: &SqlitePool) -> Result<Vec<ApiUsageByModel>> {
    let rows = sqlx::query_as::<_, (String, String, String, i64, i64, i64, i64)>(
        "SELECT model, provider, modality, \
         COALESCE(SUM(prompt_tokens), 0), COALESCE(SUM(completion_tokens), 0), \
         COALESCE(SUM(total_tokens), 0), COUNT(*) \
         FROM api_usage_log GROUP BY model, provider, modality \
         ORDER BY SUM(total_tokens) DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(model, provider, modality, pt, ct, tt, cc)| ApiUsageByModel {
            model,
            provider,
            modality,
            prompt_tokens: pt,
            completion_tokens: ct,
            total_tokens: tt,
            call_count: cc,
        })
        .collect())
}
