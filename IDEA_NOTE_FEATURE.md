# IdeaNote 灵感便签功能说明

## 功能背景

灵感便签（IdeaNote）用于快速记录突发灵感，不要求结构化，不强迫在创建时填写标题、分类、类型等信息。它是"正式词条 Entry"
的前置态，独立建模，不污染正式词条体系。

---

## 本次改动文件清单

| 文件                                  | 类型 | 说明                                  |
|-------------------------------------|----|-------------------------------------|
| `src/models/idea_note.rs`           | 新增 | IdeaNote 模型定义                       |
| `src/models/mod.rs`                 | 修改 | 注册 idea_note 模块并 re-export          |
| `src/db/traits.rs`                  | 修改 | 新增 IdeaNoteOps trait，纳入 Db 组合 trait |
| `src/db/idea_note.rs`               | 新增 | SQLite 实现                           |
| `src/db/pg_idea_note.rs`            | 新增 | PostgreSQL 实现                       |
| `src/db/mod.rs`                     | 修改 | 注册 idea_note / pg_idea_note 模块      |
| `src/lib.rs`                        | 修改 | 对外导出 IdeaNoteOps                    |
| `migrations/0003_idea_notes.sql`    | 新增 | SQLite 建表迁移                         |
| `migrations_pg/0003_idea_notes.sql` | 新增 | PostgreSQL 建表迁移                     |
| `tests/db.rs`                       | 修改 | 追加 16 个 IdeaNote 集成测试               |

---

## 数据结构

### IdeaNoteStatus 枚举

```rust
pub enum IdeaNoteStatus {
    Inbox,      // 收件箱（默认）
    Processed,  // 已处理
    Archived,   // 已归档
}
```

数据库存储为小写字符串：`"inbox"` / `"processed"` / `"archived"`。

提供 `as_str()` 和 `FromStr` 互转。

### IdeaNote 完整记录

| 字段                   | 类型               | 说明                 |
|----------------------|------------------|--------------------|
| `id`                 | `Uuid`           | 主键，UUIDv7          |
| `project_id`         | `Option<Uuid>`   | 所属项目，可为空（全局便签）     |
| `content`            | `String`         | 正文，唯一必填字段          |
| `title`              | `Option<String>` | 标题，创建时可不填          |
| `status`             | `IdeaNoteStatus` | 当前状态，默认 Inbox      |
| `pinned`             | `bool`           | 是否置顶，默认 false      |
| `created_at`         | `String`         | 创建时间               |
| `updated_at`         | `String`         | 最近更新时间（触发器自动维护）    |
| `last_reviewed_at`   | `Option<String>` | 最近回顾时间，预留字段        |
| `converted_entry_id` | `Option<Uuid>`   | 转词条后的目标词条 id，后续扩展点 |

### CreateIdeaNote

```rust
pub struct CreateIdeaNote {
    pub project_id: Option<Uuid>,  // 可为 None
    pub content: String,           // 必填
    pub title: Option<String>,     // 可为 None
    pub pinned: Option<bool>,      // 默认 false
}
```

### UpdateIdeaNote

所有字段均可选，`None` 表示不更新。部分字段使用 `Option<Option<T>>` 三态模式支持显式清空。

```rust
pub struct UpdateIdeaNote {
    pub title: Option<Option<String>>,       // Some(None) 清空
    pub content: Option<String>,
    pub status: Option<IdeaNoteStatus>,
    pub pinned: Option<bool>,
    pub last_reviewed_at: Option<Option<String>>,   // Some(None) 清空
    pub converted_entry_id: Option<Option<Uuid>>,   // Some(None) 清空
}
```

实现了 `Default`，可用 `UpdateIdeaNote { status: Some(...), ..Default::default() }` 语法。

### IdeaNoteFilter

```rust
pub struct IdeaNoteFilter<'a> {
    pub project_id: Option<&'a Uuid>,           // None = 不限项目
    pub status: Option<&'a IdeaNoteStatus>,     // None = 不限状态
    pub pinned: Option<bool>,                   // None = 不限置顶
}
```

实现了 `Default`，`IdeaNoteFilter::default()` 表示无任何过滤条件。

---

## IdeaNoteOps Trait

```rust
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
```

已纳入 `Db` 组合 trait，`SqliteDb` 和 `PgDb` 均实现。

---

## Migration 说明

### SQLite（migrations/0003_idea_notes.sql）

- 新建 `idea_notes` 表
- `project_id` 外键引用 `projects(id)`，`ON DELETE CASCADE`
- `converted_entry_id` 外键引用 `entries(id)`，`ON DELETE SET NULL`
- `status` 有 CHECK 约束，只允许 `inbox` / `processed` / `archived`
- `pinned` 存为 INTEGER 0/1，有 CHECK 约束
- `AFTER UPDATE` 触发器自动更新 `updated_at`
- 6 个索引：`project_id`, `status`, `updated_at`, `pinned`, `(project_id, status)`, `(pinned DESC, updated_at DESC)`

### PostgreSQL（migrations_pg/0003_idea_notes.sql）

- 与 SQLite 结构等价，使用 PG 原生类型
- `pinned` 为 `BOOLEAN`
- 时间字段为 `TIMESTAMPTZ`
- 使用 `BEFORE UPDATE` 触发器更新 `updated_at`

---

## 列表排序规则

`list_idea_notes` 默认排序为：

```sql
ORDER BY pinned DESC, updated_at DESC
```

置顶便签排在最前，同置顶状态内按最近更新时间倒序。

---

## 测试覆盖

| 测试名                                          | 覆盖点                            |
|----------------------------------------------|--------------------------------|
| `test_idea_note_create_and_get`              | 创建、读取、字段默认值验证                  |
| `test_idea_note_no_title_required`           | 无标题创建、pinned 默认 false          |
| `test_idea_note_get_not_found`               | 查询不存在 id 返回 NotFound           |
| `test_idea_note_update_status_and_pinned`    | 更新状态与置顶                        |
| `test_idea_note_update_title_clear`          | Some(None) 清空标题                |
| `test_idea_note_update_empty_noop`           | 空更新不修改字段                       |
| `test_idea_note_delete`                      | 删除后查询返回 NotFound               |
| `test_idea_note_delete_not_found`            | 删除不存在 id 返回 NotFound           |
| `test_idea_note_list_filter_by_project`      | 按项目过滤                          |
| `test_idea_note_list_no_project_filter`      | 无过滤返回全部                        |
| `test_idea_note_list_filter_by_status`       | 按状态过滤                          |
| `test_idea_note_list_pinned_first`           | 置顶排序验证                         |
| `test_idea_note_list_pagination`             | limit / offset 分页              |
| `test_idea_note_without_project_works`       | project_id = None 的全局便签完整 CRUD |
| `test_idea_note_cascade_delete_with_project` | 删除项目触发级联删除便签                   |
| `test_idea_note_status_parse`                | 枚举字符串互转                        |

---

## 遗留问题与后续扩展点

1. **"转词条"业务逻辑未实现**：`converted_entry_id` 字段已预留，但从 IdeaNote 创建 Entry 的逻辑需上层处理。
2. **`last_reviewed_at` 更新**：字段存在但尚无专门的"标记为已读"接口，可在 update 时传入。
3. **搜索**：当前无全文搜索支持，如有需要可参考 `entries_fts` 的做法另建 FTS 虚拟表。
4. **PG 时间戳解码**：PG 端 `created_at`/`updated_at` 等 TIMESTAMPTZ 字段在模型中以 `String` 接收，与现有 Entry 实现保持一致。
