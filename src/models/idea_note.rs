use crate::error::{Result, WorldflowError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 灵感便签状态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdeaNoteStatus {
    /// 收件箱：新建默认状态
    Inbox,
    /// 已处理：已归档或转化为词条
    Processed,
    /// 已归档：暂时搁置
    Archived,
}

impl IdeaNoteStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            IdeaNoteStatus::Inbox => "inbox",
            IdeaNoteStatus::Processed => "processed",
            IdeaNoteStatus::Archived => "archived",
        }
    }
}

impl std::str::FromStr for IdeaNoteStatus {
    type Err = WorldflowError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "inbox" => Ok(IdeaNoteStatus::Inbox),
            "processed" => Ok(IdeaNoteStatus::Processed),
            "archived" => Ok(IdeaNoteStatus::Archived),
            _ => Err(WorldflowError::InvalidInput(format!("未知便签状态: {s}"))),
        }
    }
}

/// 灵感便签完整记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeaNote {
    pub id: Uuid,
    /// 所属项目，可为空（全局便签）
    pub project_id: Option<Uuid>,
    /// 正文内容，唯一必填字段
    pub content: String,
    /// 可选标题，创建时无需填写
    pub title: Option<String>,
    /// 当前状态
    pub status: IdeaNoteStatus,
    /// 是否置顶
    pub pinned: bool,
    pub created_at: String,
    pub updated_at: String,
    /// 最近回顾时间，可为空
    pub last_reviewed_at: Option<String>,
    /// 转化为正式词条后记录目标词条 id，后续扩展点
    pub converted_entry_id: Option<Uuid>,
}

/// 创建灵感便签
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIdeaNote {
    /// 所属项目，可为空
    pub project_id: Option<Uuid>,
    /// 正文内容（必填）
    pub content: String,
    /// 可选标题
    pub title: Option<String>,
    /// 是否置顶，默认 false
    pub pinned: Option<bool>,
}

/// 更新灵感便签（所有字段可选，None 表示不更新）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateIdeaNote {
    /// None = 不更新；Some(None) = 清空标题；Some(Some(s)) = 更新为新值
    pub title: Option<Option<String>>,
    pub content: Option<String>,
    pub status: Option<IdeaNoteStatus>,
    pub pinned: Option<bool>,
    /// None = 不更新；Some(None) = 清空；Some(Some(s)) = 更新
    pub last_reviewed_at: Option<Option<String>>,
    /// None = 不更新；Some(None) = 清空；Some(Some(id)) = 更新
    pub converted_entry_id: Option<Option<Uuid>>,
}

/// 列表筛选条件
///
/// 项目筛选三态（互斥，同时指定两者会报错）：
/// - `project_id = Some(id)`：只看指定项目的便签
/// - `only_global = true`：只看 `project_id IS NULL` 的全局便签
/// - 两者均未设置：返回全部便签
#[derive(Debug, Clone, Default)]
pub struct IdeaNoteFilter<'a> {
    /// 按指定项目过滤；与 `only_global` 互斥
    pub project_id: Option<&'a Uuid>,
    /// 只返回无项目的全局便签；与 `project_id` 互斥
    pub only_global: bool,
    /// 按状态过滤
    pub status: Option<&'a IdeaNoteStatus>,
    /// 按置顶过滤
    pub pinned: Option<bool>,
}
