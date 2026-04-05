use crate::error::Result;
use crate::models::*;

pub trait ProjectOps: Send + Sync {
    async fn create_project(&self, input: CreateProject) -> Result<Project>;
    async fn get_project(&self, id: &str) -> Result<Project>;
    async fn list_projects(&self) -> Result<Vec<Project>>;
    async fn update_project(&self, id: &str, input: UpdateProject) -> Result<Project>;
    async fn delete_project(&self, id: &str) -> Result<()>;
}

pub trait CategoryOps: Send + Sync {
    async fn would_create_cycle(&self, id: &str, new_parent_id: &str) -> Result<bool>;
    async fn create_category(&self, input: CreateCategory) -> Result<Category>;
    async fn get_category(&self, id: &str) -> Result<Category>;
    async fn list_categories(&self, project_id: &str) -> Result<Vec<Category>>;
    async fn update_category(&self, id: &str, input: UpdateCategory) -> Result<Category>;
    async fn delete_category(&self, id: &str) -> Result<()>;
}

pub trait EntryOps: Send + Sync {
    async fn count_entries(&self, project_id: &str, filter: EntryFilter<'_>) -> Result<i64>;
    async fn create_entry(&self, input: CreateEntry) -> Result<Entry>;
    async fn get_entry(&self, id: &str) -> Result<Entry>;
    async fn list_entries(&self, project_id: &str, filter: EntryFilter<'_>, limit: usize, offset: usize) -> Result<Vec<EntryBrief>>;
    async fn search_entries(&self, project_id: &str, query: &str, filter: EntryFilter<'_>, limit: usize) -> Result<Vec<EntryBrief>>;
    async fn update_entry(&self, id: &str, input: UpdateEntry) -> Result<Entry>;
    async fn delete_entry(&self, id: &str) -> Result<()>;
    async fn create_entries_bulk(&self, inputs: Vec<CreateEntry>) -> Result<usize>;
}

pub trait TagSchemaOps: Send + Sync {
    async fn create_tag_schema(&self, input: CreateTagSchema) -> Result<TagSchema>;
    async fn get_tag_schema(&self, id: &str) -> Result<TagSchema>;
    async fn list_tag_schemas(&self, project_id: &str) -> Result<Vec<TagSchema>>;
    async fn update_tag_schema(&self, id: &str, input: CreateTagSchema) -> Result<TagSchema>;
    async fn delete_tag_schema(&self, id: &str) -> Result<()>;
}

pub trait EntryRelationOps: Send + Sync {
    async fn create_relation(&self, input: CreateEntryRelation) -> Result<EntryRelation>;
    async fn get_relation(&self, id: &str) -> Result<EntryRelation>;
    async fn list_relations_for_entry(&self, entry_id: &str) -> Result<Vec<EntryRelation>>;
    async fn list_relations_for_project(&self, project_id: &str) -> Result<Vec<EntryRelation>>;
    async fn update_relation(&self, id: &str, input: UpdateEntryRelation) -> Result<EntryRelation>;
    async fn delete_relation(&self, id: &str) -> Result<()>;
    async fn delete_relations_between(&self, entry_a: &str, entry_b: &str) -> Result<u64>;
}

pub trait EntryTypeOps: Send + Sync {
    /// 创建自定义词条类型
    async fn create_entry_type(&self, input: CreateCustomEntryType) -> Result<CustomEntryType>;

    /// 获取自定义词条类型
    async fn get_entry_type(&self, id: &str) -> Result<CustomEntryType>;

    /// 列出项目内所有词条类型（内置+自定义，内置在前）
    async fn list_all_entry_types(&self, project_id: &str) -> Result<Vec<EntryTypeView>>;

    /// 列出项目内自定义词条类型
    async fn list_custom_entry_types(&self, project_id: &str) -> Result<Vec<CustomEntryType>>;

    /// 更新自定义词条类型
    async fn update_entry_type(&self, id: &str, input: UpdateCustomEntryType) -> Result<CustomEntryType>;

    /// 删除自定义词条类型（前提：entries 中不存在该 type 的引用）
    async fn delete_entry_type(&self, id: &str) -> Result<()>;

    /// 检查是否有 entries 在使用该 type
    async fn check_entry_type_in_use(&self, project_id: &str, type_id: &str) -> Result<bool>;
}

/// 组合 trait，实现所有子 trait 即自动实现 Db
pub trait Db: ProjectOps + CategoryOps + EntryOps + TagSchemaOps + EntryRelationOps + EntryTypeOps {}
impl<T: ProjectOps + CategoryOps + EntryOps + TagSchemaOps + EntryRelationOps + EntryTypeOps> Db for T {}
