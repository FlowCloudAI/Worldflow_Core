use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RelationDirection {
    OneWay,
    TwoWay,
}

impl RelationDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            RelationDirection::OneWay => "one_way",
            RelationDirection::TwoWay => "two_way",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "one_way" => Some(RelationDirection::OneWay),
            "two_way" => Some(RelationDirection::TwoWay),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryRelation {
    pub id:         String,
    pub project_id: String,
    pub a_id:       String,
    pub b_id:       String,
    pub relation:   RelationDirection,
    pub content:    String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEntryRelation {
    pub project_id: String,
    pub a_id:       String,
    pub b_id:       String,
    pub relation:   RelationDirection,
    pub content:    String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEntryRelation {
    pub relation: Option<RelationDirection>,
    pub content:  Option<String>,
}