use crate::error::{Result, WorldflowError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 内置词条类型定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltinEntryType {
    pub key: &'static str,         // 短标识符，如 "character"
    pub name: &'static str,        // 显示名称，如 "人物"
    pub description: &'static str, // 描述信息
    pub icon: &'static str,        // 图标（emoji 或 class 名称）
    pub color: &'static str,       // 颜色（hex 或 rgb）
}

/// 9 个内置词条类型常量数组
pub const BUILTIN_ENTRY_TYPES: &[BuiltinEntryType] = &[
    BuiltinEntryType {
        key: "character",
        name: "人物",
        description: "故事中的人物角色",
        icon: "👤",
        color: "#3B82F6",
    },
    BuiltinEntryType {
        key: "organization",
        name: "组织",
        description: "团体、势力、国家等组织",
        icon: "🏢",
        color: "#8B5CF6",
    },
    BuiltinEntryType {
        key: "location",
        name: "地点",
        description: "地理位置、场景、地域",
        icon: "📍",
        color: "#EC4899",
    },
    BuiltinEntryType {
        key: "item",
        name: "物品",
        description: "道具、装备、物件",
        icon: "📦",
        color: "#F59E0B",
    },
    BuiltinEntryType {
        key: "creature",
        name: "生物",
        description: "非人类生物、怪物、动植物",
        icon: "🦁",
        color: "#10B981",
    },
    BuiltinEntryType {
        key: "event",
        name: "事件",
        description: "情节、历史事件、发生的事情",
        icon: "⚡",
        color: "#EF4444",
    },
    BuiltinEntryType {
        key: "concept",
        name: "概念",
        description: "设定、规则、理论、体系",
        icon: "💡",
        color: "#06B6D4",
    },
    BuiltinEntryType {
        key: "culture",
        name: "文化",
        description: "文化、风俗、传统、习俗",
        icon: "🎭",
        color: "#A78BFA",
    },
    BuiltinEntryType {
        key: "else",
        name: "其他",
        description: "其他不分类的词条",
        icon: "📄",
        color: "#6B7280",
    },
];

/// 自定义词条类型
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CustomEntryType {
    pub id: Uuid, // UUID
    pub project_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 创建自定义词条类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCustomEntryType {
    pub project_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
}

/// 更新自定义词条类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCustomEntryType {
    pub name: Option<String>,
    pub description: Option<Option<String>>, // Option<Option<T>> 模式：None = 不更新，Some(None) = 清空
    pub icon: Option<Option<String>>,
    pub color: Option<Option<String>>,
}

/// 词条类型视图（统一展示内置和自定义类型）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EntryTypeView {
    Builtin {
        key: &'static str,
        name: &'static str,
        description: &'static str,
        icon: &'static str,
        color: &'static str,
    },
    Custom(CustomEntryType),
}

impl EntryTypeView {
    /// 返回类型的 key（内置）或 id（自定义）
    pub fn key_or_id(&self) -> String {
        match self {
            EntryTypeView::Builtin { key, .. } => (*key).to_string(),
            EntryTypeView::Custom(ct) => ct.id.to_string(),
        }
    }

    /// 返回显示名称
    pub fn name(&self) -> &str {
        match self {
            EntryTypeView::Builtin { name, .. } => name,
            EntryTypeView::Custom(ct) => &ct.name,
        }
    }

    /// 返回图标
    pub fn icon(&self) -> &str {
        match self {
            EntryTypeView::Builtin { icon, .. } => icon,
            EntryTypeView::Custom(ct) => ct.icon.as_deref().unwrap_or("📄"),
        }
    }

    /// 返回颜色
    pub fn color(&self) -> &str {
        match self {
            EntryTypeView::Builtin { color, .. } => color,
            EntryTypeView::Custom(ct) => ct.color.as_deref().unwrap_or("#6B7280"),
        }
    }
}

/// 从 BuiltinEntryType 转换为 EntryTypeView
impl From<&'static BuiltinEntryType> for EntryTypeView {
    fn from(bt: &'static BuiltinEntryType) -> Self {
        EntryTypeView::Builtin {
            key: bt.key,
            name: bt.name,
            description: bt.description,
            icon: bt.icon,
            color: bt.color,
        }
    }
}

/// 从 CustomEntryType 转换为 EntryTypeView
impl From<CustomEntryType> for EntryTypeView {
    fn from(ct: CustomEntryType) -> Self {
        EntryTypeView::Custom(ct)
    }
}

/// 判断字符串是否为内置类型 key（无法解析为 UUID 的视为内置 key）
pub fn is_builtin_type(s: &str) -> bool {
    Uuid::try_parse(s).is_err()
}

/// 获取内置类型定义
pub fn get_builtin_type(key: &str) -> Option<&'static BuiltinEntryType> {
    BUILTIN_ENTRY_TYPES.iter().find(|t| t.key == key)
}

/// 验证内置类型 key 的有效性
pub fn validate_builtin_type_key(key: &str) -> Result<()> {
    if get_builtin_type(key).is_none() {
        return Err(WorldflowError::InvalidInput(format!(
            "Invalid builtin entry type: {}",
            key
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_entry_types_count() {
        assert_eq!(BUILTIN_ENTRY_TYPES.len(), 9);
    }

    #[test]
    fn test_builtin_entry_types_keys_unique() {
        let mut keys: Vec<&str> = BUILTIN_ENTRY_TYPES.iter().map(|t| t.key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), 9, "All builtin keys should be unique");
    }

    #[test]
    fn test_is_builtin_type_detection() {
        assert!(is_builtin_type("character"));
        assert!(is_builtin_type("organization"));
        assert!(!is_builtin_type("018f0d4e-6b30-7c2a-9f65-8d7b3a1c2e4f")); // UUID
    }

    #[test]
    fn test_get_builtin_type() {
        let ct = get_builtin_type("character");
        assert!(ct.is_some());
        assert_eq!(ct.unwrap().name, "人物");

        let invalid = get_builtin_type("invalid_type");
        assert!(invalid.is_none());
    }

    #[test]
    fn test_validate_builtin_type_key() {
        assert!(validate_builtin_type_key("character").is_ok());
        assert!(validate_builtin_type_key("invalid_type").is_err());
    }

    #[test]
    fn test_entry_type_view_from_builtin() {
        let builtin = &BUILTIN_ENTRY_TYPES[0];
        let view: EntryTypeView = builtin.into();
        assert_eq!(view.key_or_id(), "character");
        assert_eq!(view.name(), "人物");
    }

    #[test]
    fn test_entry_type_view_from_custom() {
        let custom_id = Uuid::parse_str("018f0d4e-6b30-7c2a-9f65-8d7b3a1c2e4f").unwrap();
        let custom = CustomEntryType {
            id: custom_id,
            project_id: Uuid::now_v7(),
            name: "自定义类型".to_string(),
            description: Some("描述".to_string()),
            icon: Some("🎨".to_string()),
            color: Some("#FF0000".to_string()),
            created_at: "2026-04-04T00:00:00".to_string(),
            updated_at: "2026-04-04T00:00:00".to_string(),
        };
        let view: EntryTypeView = custom.into();
        assert_eq!(view.key_or_id(), "018f0d4e-6b30-7c2a-9f65-8d7b3a1c2e4f");
        assert_eq!(view.name(), "自定义类型");
    }
}
