use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntryLink {
    pub id: Uuid,
    pub project_id: Uuid,
    pub a_id: Uuid,
    pub b_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateEntryLink {
    pub project_id: Uuid,
    pub a_id: Uuid,
    pub b_id: Uuid,
}
