use crate::error::Result;
use crate::models::*;
use uuid::Uuid;

pub trait ProjectOps: Send + Sync {
    async fn create_project(&self, input: CreateProject) -> Result<Project>;
    async fn get_project(&self, id: &Uuid) -> Result<Project>;
    async fn list_projects(&self) -> Result<Vec<Project>>;
    async fn update_project(&self, id: &Uuid, input: UpdateProject) -> Result<Project>;
    async fn delete_project(&self, id: &Uuid) -> Result<()>;
}

pub trait CategoryOps: Send + Sync {
    async fn would_create_cycle(&self, id: &Uuid, new_parent_id: &Uuid) -> Result<bool>;
    async fn create_category(&self, input: CreateCategory) -> Result<Category>;
    async fn create_categories_bulk(&self, inputs: Vec<CreateCategory>) -> Result<Vec<Category>>;
    async fn get_category(&self, id: &Uuid) -> Result<Category>;
    async fn list_categories(&self, project_id: &Uuid) -> Result<Vec<Category>>;
    async fn update_category(&self, id: &Uuid, input: UpdateCategory) -> Result<Category>;
    async fn delete_category(&self, id: &Uuid) -> Result<()>;
}

pub trait EntryOps: Send + Sync {
    async fn count_entries(&self, project_id: &Uuid, filter: EntryFilter<'_>) -> Result<i64>;
    async fn create_entry(&self, input: CreateEntry) -> Result<Entry>;
    async fn get_entry(&self, id: &Uuid) -> Result<Entry>;
    async fn list_entries(
        &self,
        project_id: &Uuid,
        filter: EntryFilter<'_>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<EntryBrief>>;
    async fn search_entries(
        &self,
        project_id: &Uuid,
        query: &str,
        filter: EntryFilter<'_>,
        limit: usize,
    ) -> Result<Vec<EntryBrief>>;
    async fn update_entry(&self, id: &Uuid, input: UpdateEntry) -> Result<Entry>;
    async fn delete_entry(&self, id: &Uuid) -> Result<()>;
    async fn create_entries_bulk(&self, inputs: Vec<CreateEntry>) -> Result<usize>;
}

pub trait TagSchemaOps: Send + Sync {
    async fn create_tag_schema(&self, input: CreateTagSchema) -> Result<TagSchema>;
    async fn create_tag_schemas_bulk(&self, inputs: Vec<CreateTagSchema>)
    -> Result<Vec<TagSchema>>;
    async fn get_tag_schema(&self, id: &Uuid) -> Result<TagSchema>;
    async fn list_tag_schemas(&self, project_id: &Uuid) -> Result<Vec<TagSchema>>;
    async fn update_tag_schema(&self, id: &Uuid, input: CreateTagSchema) -> Result<TagSchema>;
    async fn delete_tag_schema(&self, id: &Uuid) -> Result<()>;
}

pub trait EntryRelationOps: Send + Sync {
    async fn create_relation(&self, input: CreateEntryRelation) -> Result<EntryRelation>;
    async fn create_relations_bulk(
        &self,
        inputs: Vec<CreateEntryRelation>,
    ) -> Result<Vec<EntryRelation>>;
    async fn get_relation(&self, id: &Uuid) -> Result<EntryRelation>;
    async fn list_relations_for_entry(&self, entry_id: &Uuid) -> Result<Vec<EntryRelation>>;
    async fn list_relations_for_project(&self, project_id: &Uuid) -> Result<Vec<EntryRelation>>;
    async fn update_relation(&self, id: &Uuid, input: UpdateEntryRelation)
    -> Result<EntryRelation>;
    async fn delete_relation(&self, id: &Uuid) -> Result<()>;
    async fn delete_relations_between(&self, entry_a: &Uuid, entry_b: &Uuid) -> Result<u64>;
}

pub trait EntryLinkOps: Send + Sync {
    async fn create_link(&self, input: CreateEntryLink) -> Result<EntryLink>;
    async fn list_outgoing_links(&self, entry_id: &Uuid) -> Result<Vec<EntryLink>>;
    async fn list_incoming_links(&self, entry_id: &Uuid) -> Result<Vec<EntryLink>>;
    async fn delete_links_from_entry(&self, entry_id: &Uuid) -> Result<u64>;
    async fn replace_outgoing_links(
        &self,
        project_id: &Uuid,
        entry_id: &Uuid,
        linked_entry_ids: &[Uuid],
    ) -> Result<Vec<EntryLink>>;
}

pub trait EntryTypeOps: Send + Sync {
    /// 创建自定义词条类型
    async fn create_entry_type(&self, input: CreateCustomEntryType) -> Result<CustomEntryType>;
    async fn create_entry_types_bulk(
        &self,
        inputs: Vec<CreateCustomEntryType>,
    ) -> Result<Vec<CustomEntryType>>;

    /// 获取自定义词条类型
    async fn get_entry_type(&self, id: &Uuid) -> Result<CustomEntryType>;

    /// 列出项目内所有词条类型（内置+自定义，内置在前）
    async fn list_all_entry_types(&self, project_id: &Uuid) -> Result<Vec<EntryTypeView>>;

    /// 列出项目内自定义词条类型
    async fn list_custom_entry_types(&self, project_id: &Uuid) -> Result<Vec<CustomEntryType>>;

    /// 更新自定义词条类型
    async fn update_entry_type(
        &self,
        id: &Uuid,
        input: UpdateCustomEntryType,
    ) -> Result<CustomEntryType>;

    /// 删除自定义词条类型（前提：entries 中不存在该 type 的引用）
    async fn delete_entry_type(&self, id: &Uuid) -> Result<()>;

    /// 检查是否有 entries 在使用该 type
    async fn check_entry_type_in_use(&self, project_id: &Uuid, type_id: &Uuid) -> Result<bool>;
}

pub trait IdeaNoteOps: Send + Sync {
    async fn create_idea_note(&self, input: CreateIdeaNote) -> Result<IdeaNote>;
    async fn get_idea_note(&self, id: &Uuid) -> Result<IdeaNote>;
    async fn list_idea_notes(
        &self,
        filter: IdeaNoteFilter<'_>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<IdeaNote>>;
    async fn update_idea_note(&self, id: &Uuid, input: UpdateIdeaNote) -> Result<IdeaNote>;
    async fn delete_idea_note(&self, id: &Uuid) -> Result<()>;
}

/// 组合 trait，实现所有子 trait 即自动实现 Db
pub trait Db:
    ProjectOps
    + CategoryOps
    + EntryOps
    + TagSchemaOps
    + EntryRelationOps
    + EntryLinkOps
    + EntryTypeOps
    + IdeaNoteOps
{
}
impl<
    T: ProjectOps
        + CategoryOps
        + EntryOps
        + TagSchemaOps
        + EntryRelationOps
        + EntryLinkOps
        + EntryTypeOps
        + IdeaNoteOps,
> Db for T
{
}
