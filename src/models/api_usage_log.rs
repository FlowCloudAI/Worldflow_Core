use serde::{Deserialize, Serialize};

/// API 调用模态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiModality {
    Llm,
    Image,
    Tts,
}

impl ApiModality {
    pub fn as_str(&self) -> &'static str {
        match self {
            ApiModality::Llm => "llm",
            ApiModality::Image => "image",
            ApiModality::Tts => "tts",
        }
    }
}

/// API 用量记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiUsageLog {
    pub id: String,
    pub session_id: String,
    pub model: String,
    pub provider: String,
    pub modality: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub created_at: String,
}

/// 创建用量记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateApiUsageLog {
    pub id: String,
    pub session_id: String,
    pub model: String,
    pub provider: String,
    pub modality: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

/// 用量统计聚合
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiUsageSummary {
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_tokens: i64,
    pub call_count: i64,
}

/// 按模型分组的用量统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiUsageByModel {
    pub model: String,
    pub provider: String,
    pub modality: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub call_count: i64,
}
