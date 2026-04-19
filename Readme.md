# worldflow_core

`worldflow_core` 是一个 Rust 库，负责世界观/设定数据的核心存储层。

它当前提供 8 组核心能力：

- `Project`: 顶层项目容器
- `Category`: 树状分类
- `Entry`: 词条内容
- `TagSchema`: 标签定义
- `EntryLink`: 基于稳定 id 的词条内部链接
- `EntryRelation`: 词条关系
- `EntryType`: 9 个内置类型 + 项目级自定义类型
- `IdeaNote`: 灵感便签，Entry 的前置态，独立建模

以及 1 个 SQLite 专属扩展：

- `Snapshot`：伪数据库版本管理，全量导出 CSV + git2 管理历史，支持回退和追加恢复

默认后端是 SQLite，可选启用 PostgreSQL。

---

## 给 Codex 的 60 秒接管版

如果你是来接手这个仓库的，先记住这几件事：

- 这是库，不是可执行程序；入口是 [src/lib.rs](src/lib.rs)。
- 默认 feature 是 `sqlite`；只开 PostgreSQL 时要用 `--no-default-features --features postgres`。
- 所有业务接口都通过 traits 暴露，定义在 [src/db/traits.rs](src/db/traits.rs)。
- SQLite 实现在 [src/db](src/db)，PostgreSQL 实现在同目录的 `pg_*.rs`。
- SQLite 初始化会自动开 `foreign_keys`、`WAL`、`synchronous=NORMAL`、自适应内存参数，并自动跑迁移。
- PostgreSQL 初始化也会自动跑迁移。
- 集成测试目前没有按 feature 做门控，`cargo test` 不能当成“仓库当前必过”的命令。

建议的阅读顺序：

1. [src/lib.rs](src/lib.rs)
2. [src/db/traits.rs](src/db/traits.rs)
3. [src/models](src/models)
4. [src/db/mod.rs](src/db/mod.rs)
5. [migrations/0001_init.sql](migrations/0001_init.sql)
6. [migrations_pg/0001_init.sql](migrations_pg/0001_init.sql)

---

## 仓库定位

这个仓库专注于“设定数据层”，不是完整的多人协作应用。它负责：

- 结构化存储项目、分类、词条、标签、关系、词条类型
- 提供 SQLite / PostgreSQL 两套后端实现
- 通过统一 traits 暴露 CRUD、搜索、过滤、批量写入能力
- 通过迁移和触发器维护数据一致性

它目前不提供：

- 用户系统
- 权限系统
- 乐观锁/冲突解决机制
- HTTP API
- 前端界面

如果上层要做多人协作、权限、审计、版本历史，需要在本库之上继续封装。

---

## 当前能力总览

| 模块            | 核心结构                                                            | 说明                                         |
|---------------|-----------------------------------------------------------------|--------------------------------------------|
| Project       | `Project` / `CreateProject` / `UpdateProject`                   | 项目顶层容器，带可选封面                               |
| Category      | `Category` / `CreateCategory` / `UpdateCategory`                | 树状分类，支持循环检测                                |
| Entry         | `Entry` / `EntryBrief` / `CreateEntry` / `UpdateEntry`          | 核心词条内容，带标签、图片、类型                           |
| TagSchema     | `TagSchema` / `CreateTagSchema`                                 | 标签定义；更新接口也复用 `CreateTagSchema`             |
| EntryLink     | `EntryLink` / `CreateEntryLink`                                 | 词条内部链接，语义为 `a_id -> b_id`，用于正向链接、反向链接、实时同步 |
| EntryRelation | `EntryRelation` / `CreateEntryRelation` / `UpdateEntryRelation` | 词条关系，支持单向/双向                               |
| EntryType     | `BuiltinEntryType` / `CustomEntryType` / `EntryTypeView`        | 9 个内置类型 + 项目级自定义类型                         |
| IdeaNote      | `IdeaNote` / `CreateIdeaNote` / `UpdateIdeaNote`                | 灵感便签，快速记录突发灵感，Entry 的前置态，独立建模              |

后端能力差异：

| 项目     | SQLite                                    | PostgreSQL                                                             |
|--------|-------------------------------------------|------------------------------------------------------------------------|
| 默认启用   | 是                                         | 否                                                                      |
| 连接入口   | `SqliteDb::new()` / `new_with_snapshot()` | `PgDb::new()`                                                          |
| 迁移目录   | `migrations/`                             | `migrations_pg/`                                                       |
| 搜索实现   | FTS5 `trigram`，索引 `title + content`       | `to_tsvector('simple', title + summary + content)` + `plainto_tsquery` |
| FTS 维护 | `optimize_fts()`                          | 无                                                                      |
| 连接池上限  | 5                                         | 10                                                                     |
| 版本快照   | 有（CSV + git2）                             | 无                                                                      |

---

## 实际目录结构

下面是当前仓库里真正有用的部分，不是概念图：

```text
worldflow_core/
├── Cargo.toml
├── migrations/
│   └── 0001_init.sql
│   └── 0002_entry_links.sql
│   └── 0003_idea_notes.sql
├── migrations_pg/
│   └── 0001_init.sql
│   └── 0002_entry_links.sql
│   └── 0003_idea_notes.sql
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── models/
│   │   ├── mod.rs
│   │   ├── project.rs
│   │   ├── category.rs
│   │   ├── entry.rs
│   │   ├── entry_link.rs
│   │   ├── tag_schema.rs
│   │   ├── entry_relation.rs
│   │   ├── entry_type.rs
│   │   └── idea_note.rs
│   └── db/
│       ├── mod.rs
│       ├── traits.rs
│       ├── snapshot.rs          ← SQLite 专属，版本管理
│       ├── sqlite.rs
│       ├── postgres.rs
│       ├── project.rs
│       ├── category.rs
│       ├── entry.rs
│       ├── entry_link.rs
│       ├── tag_schema.rs
│       ├── entry_relation.rs
│       ├── entry_type.rs
│       ├── pg_project.rs
│       ├── pg_category.rs
│       ├── pg_entry.rs
│       ├── pg_entry_link.rs
│       ├── pg_tag_schema.rs
│       ├── pg_entry_relation.rs
│       ├── pg_entry_type.rs
│       ├── idea_note.rs
│       └── pg_idea_note.rs
└── tests/
    ├── db.rs
    ├── snapshot.rs              ← snapshot 功能集成测试（14 个用例）
    ├── stress_test.rs
    └── stress_test_pg.rs
```

---

## 依赖与环境

当前 crate 配置见 [Cargo.toml](Cargo.toml)：

- edition: `2024`
- 默认 feature: `sqlite`
- 可选 feature: `postgres`

因此本地开发建议：

- 使用支持 Rust 2024 edition 的稳定版工具链
- 如果要跑 PostgreSQL 相关内容，需要显式启用 `postgres` feature

示例依赖写法：

```toml
[dependencies]
worldflow_core = { path = "../worldflow_core" }
tokio = { version = "1", features = ["full"] }
```

如果你只想用 PostgreSQL：

```toml
[dependencies]
worldflow_core = { path = "../worldflow_core", default-features = false, features = ["postgres"] }
tokio = { version = "1", features = ["full"] }
```

如果你想同时保留两种后端能力：

```toml
[dependencies]
worldflow_core = { path = "../worldflow_core", features = ["postgres"] }
tokio = { version = "1", features = ["full"] }
```

说明：

- 默认已经包含 `sqlite`
- 再加 `features = ["postgres"]` 就是“SQLite + PostgreSQL 都可用”

---

## 最小可用示例

### SQLite

这个示例按当前 API 可编译，不依赖 README 里的旧字段名。

```rust
use worldflow_core::{
    models::*,
    EntryOps, ProjectOps, Result, SqliteDb,
};

#[tokio::main]
async fn main() -> Result<()> {
    let db = SqliteDb::new("sqlite:./worldflow_dev.db").await?;

    let project = db.create_project(CreateProject {
        name: "我的世界观".to_string(),
        description: Some("一个测试项目".to_string()),
        cover_image: None,
    }).await?;

    let entry = db.create_entry(CreateEntry {
        project_id: project.id.clone(),
        category_id: None,
        title: "主角".to_string(),
        summary: Some("故事主人公".to_string()),
        content: Some("详细设定内容".to_string()),
        r#type: Some("character".to_string()),
        tags: None,
        images: None,
    }).await?;

    let hits = db.search_entries(&project.id, "主角", EntryFilter::default(), 10).await?;

    println!("entry_id={}", entry.id);
    println!("hits={}", hits.len());
    Ok(())
}
```

### PostgreSQL

```rust
use worldflow_core::{
    models::*,
    EntryOps, ProjectOps, Result, PgDb,
};

#[tokio::main]
async fn main() -> Result<()> {
    let db = PgDb::new("postgres://postgres:password@localhost/worldflow_dev").await?;

    let project = db.create_project(CreateProject {
        name: "PG 世界观".to_string(),
        description: None,
        cover_image: None,
    }).await?;

    let _entry = db.create_entry(CreateEntry {
        project_id: project.id.clone(),
        category_id: None,
        title: "地点 A".to_string(),
        summary: Some("测试用词条".to_string()),
        content: Some("这里是内容".to_string()),
        r#type: Some("location".to_string()),
        tags: None,
        images: None,
    }).await?;

    Ok(())
}
```

重要：

- 方法不是直接定义在 struct 固有 impl 上，而是来自 traits
- 所以示例里必须把 `ProjectOps`、`EntryOps` 等 trait 引入作用域

---

## 你最容易踩到的 API 细节

### 1. `CreateEntry` 的类型字段叫 `r#type`

不是 `entry_type`。

实际定义在 [src/models/entry.rs](src/models/entry.rs)：

```rust
pub struct CreateEntry {
    pub project_id: String,
    pub category_id: Option<String>,
    pub title: String,
    pub summary: Option<String>,
    pub content: Option<String>,
    pub r#type: Option<String>,
    pub tags: Option<Vec<EntryTag>>,
    pub images: Option<Vec<FCImage>>,
}
```

### 2. `UpdateEntry.summary` 不能表达“清空为 NULL”

当前定义是：

```rust
pub struct UpdateEntry {
    pub category_id: Option<Option<String>>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub content: Option<String>,
    pub r#type: Option<Option<String>>,
    pub tags: Option<Vec<EntryTag>>,
    pub images: Option<Vec<FCImage>>,
}
```

这意味着：

- `summary: None` 表示“不更新”
- 没有 `Some(None)` 这种表达能力
- 也就是说，当前 API 不能显式把 `summary` 清空成数据库 `NULL`

### 3. `UpdateProject.cover_image` / `UpdateCategory.parent_id` / `UpdateEntry.category_id` / `UpdateEntry.r#type` 使用 `Option<Option<T>>`

这几个字段有三态语义：

- `None`: 不更新
- `Some(Some(v))`: 更新为新值
- `Some(None)`: 显式清空

示例：

```rust
let updated = db.update_project(&project.id, UpdateProject {
    name: None,
    description: None,
    cover_image: Some(None),
}).await?;
```

### 4. `TagSchema` 没有单独的 `UpdateTagSchema`

更新接口仍然是：

```rust
async fn update_tag_schema(&self, id: &str, input: CreateTagSchema) -> Result<TagSchema>;
```

也就是说，更新标签定义时需要提供完整的 `CreateTagSchema` 结构。

### 5. `EntryFilter` 目前只有两个条件

```rust
#[derive(Debug, Clone, Default)]
pub struct EntryFilter<'a> {
    pub category_id: Option<&'a str>,
    pub entry_type: Option<&'a str>,
}
```

没有更多组合查询 DSL。

---

## 数据模型与行为说明

### Project

项目是顶级隔离单元。

字段重点：

- `id: String`
- `name: String`
- `description: Option<String>`
- `cover_image: Option<String>`

支持：

- 创建
- 单个查询
- 列表查询
- 更新
- 删除

删除项目会级联删除该项目下的：

- categories
- entries
- entry_links
- tag_schemas
- entry_relations
- entry_types

### Category

分类是树状结构，支持无限层级。

关键行为：

- `would_create_cycle()` 会检查把某节点移动到新父节点下是否形成环
- 应用层在 `update_category()` 时显式调用该检查

删除分类的真实行为：

- 子分类会级联删除
- 引用这些分类的词条会因为 `entries.category_id ON DELETE SET NULL` 而失去分类绑定

这不是“需要手动处理子分类”的实现。

### Entry

词条有两个主要视图：

- `Entry`: 完整记录
- `EntryBrief`: 列表优化版本，不带完整 `content`/`tags`

与存储相关的事实：

- `tags` 存成 JSON 文本
- `images` 存成 JSON 文本
- `cover_path` 从 `images` 中 `is_cover = true` 的图片路径提取
- `content: None` 创建时会落成空字符串

### TagSchema

标签定义用于描述词条标签结构。

当前约束：

- `type` 只能是 `number` / `string` / `boolean`
- `default_val` 会做基础校验
- `range_min <= range_max`

注意：

- `target` 在模型层输入是 `Vec<String>`，落库后是 JSON 字符串

### EntryRelation

支持两种方向：

- `one_way`
- `two_way`

行为要点：

- `a_id != b_id`
- 关系必须属于同一个项目
- 双向关系会在应用层被规范成 `a_id < b_id`
- SQLite 迁移里对乱序双向关系是触发器拒绝
- PostgreSQL 迁移里对乱序双向关系是触发器自动交换

### EntryType

当前有 9 个内置类型：

- `character`
- `organization`
- `location`
- `item`
- `creature`
- `event`
- `concept`
- `culture`
- `else`

自定义类型存于 `entry_types` 表。

实际接口：

- `create_entry_type`
- `get_entry_type`
- `list_all_entry_types`
- `list_custom_entry_types`
- `update_entry_type`
- `delete_entry_type`
- `check_entry_type_in_use`

约定：

- 内置类型在内存中定义，不入库
- 词条使用内置类型时，`Entry.r#type` 存 key
- 词条使用自定义类型时，`Entry.r#type` 存 UUID 字符串

---

## 搜索、索引与性能

### SQLite

`SqliteDb::new()` 当前做这些事：

1. 创建连接池，`max_connections = 5`
2. `PRAGMA foreign_keys = ON`
3. `PRAGMA journal_mode = WAL`
4. `PRAGMA synchronous = NORMAL`
5. 自动执行 `migrations/`
6. 根据可用内存设置 `cache_size` / `mmap_size` / `temp_store`

SQLite 搜索实现：

- 使用 FTS5 虚拟表 `entries_fts`
- tokenizer 是 `trigram`
- 当前索引字段是 `title + content`
- `summary` 没进 SQLite FTS 索引

批量写入后可以手动做一次：

```rust
db.optimize_fts().await?;
```

### PostgreSQL

`PgDb::new()` 当前做这些事：

1. 创建连接池，`max_connections = 10`
2. 自动执行 `migrations_pg/`

PostgreSQL 搜索实现：

- `to_tsvector('simple', title || summary || content)`
- `plainto_tsquery('simple', $query)`

因此 README 不应该宣称“明确支持 AND/OR/NOT 布尔查询语法”。按当前实现，查询字符串会按 `plainto_tsquery` 规则处理。

### 常用索引

SQLite 与 PostgreSQL 都有这些主干索引：

- entries by `project_id`
- entries by `category_id`
- entries by `type`
- categories by `project_id`
- tag_schemas by `project_id`
- relations by `a_id`
- relations by `b_id`
- relations by `project_id`
- entry_links by `a_id`
- entry_links by `b_id`
- entry_links by `project_id`
- entry_types by `project_id`

---

## 迁移与数据库事实

SQLite 迁移文件：

- [migrations/0001_init.sql](migrations/0001_init.sql)
- [migrations/0002_entry_links.sql](migrations/0002_entry_links.sql)

PostgreSQL 迁移文件：

- [migrations_pg/0001_init.sql](migrations_pg/0001_init.sql)
- [migrations_pg/0002_entry_links.sql](migrations_pg/0002_entry_links.sql)

当前迁移已经包含：

- 表创建
- 触发器
- 索引
- FTS/tsvector 相关结构
- `entry_links` 表
- `entry_types` 表

你不需要手动先跑 `sqlx migrate run` 才能使用库，只要调用 `SqliteDb::new()` 或 `PgDb::new()`，库会自动执行迁移。

如果你只是用 SQLx CLI 做额外维护，那才需要手动命令。

---

## Snapshot 版本管理

详见 [Snapshot.md](Snapshot.md)。

---

## 面向 Codex 的修改建议

如果你接到需求要改这个仓库，通常按下面套路最快：

### 新增字段

1. 改对应模型
2. 改 SQLite / PG 的 row mapping
3. 改 `create_*` / `get_*` / `list_*` / `update_*`
4. 改两套迁移
5. 改 README 示例
6. 补至少一个单元测试或集成测试

### 新增模块

1. 在 [src/models/mod.rs](src/models/mod.rs) 注册模型
2. 在 [src/db/traits.rs](src/db/traits.rs) 加 trait 方法
3. 实现 SQLite 版本
4. 实现 PostgreSQL 版本
5. 在 [src/db/mod.rs](src/db/mod.rs) 注册模块
6. 写迁移
7. 更新 README

---

## 词条内部链接改造说明

当前库同时保留两套能力：

- 旧正文内容仍可继续保存基于标题的内部链接写法，库层不会主动改写历史正文
- 新增 `entry_links` 表，用稳定 `id` 维护结构化内部链接，语义固定为 `a_id -> b_id`

推荐的上层用法：

1. 保存词条正文时，继续保留原始内容，避免一次性破坏旧数据
2. 同步解析正文中的内部链接，并把最新出链写入 `entry_links`
3. 读取正向链接时按 `a_id` 查 `entry_links`
4. 读取反向链接时按 `b_id` 查 `entry_links`
5. 老正文里仍然只有标题链接的内容，可继续由上层按标题做兜底解析

当前 `EntryLinkOps` 提供的核心接口：

- `list_outgoing_links(entry_id)`
- `list_incoming_links(entry_id)`
- `delete_links_from_entry(entry_id)`
- `replace_outgoing_links(project_id, entry_id, linked_entry_ids)`

`replace_outgoing_links` 会先删旧出链，再批量写入当前词条的最新出链，并自动过滤重复目标和自链接。

### 改搜索

先确认你到底想改哪一端：

- SQLite 搜索在 [src/db/entry.rs](src/db/entry.rs)
- PostgreSQL 搜索在 [src/db/pg_entry.rs](src/db/pg_entry.rs)

这两套实现现在不是完全等价的，尤其是：

- SQLite 不搜 `summary`
- PostgreSQL 搜 `summary`
- PostgreSQL 用 `plainto_tsquery`

---

## 实用命令

### 只检查库本体

默认 SQLite：

```bash
cargo check --lib
```

只检查 PostgreSQL 版本：

```bash
cargo check --lib --no-default-features --features postgres
```

### 只跑库内单元测试

默认 SQLite：

```bash
cargo test --lib
```

只跑 PostgreSQL feature 下的库内单元测试：

```bash
cargo test --lib --no-default-features --features postgres
```

### SQLx CLI

只有在你需要手动管理数据库时再用：

```bash
cargo install sqlx-cli
sqlx migrate run
```

---

## 当前测试现状

仓库里有 4 个集成测试文件：

- [tests/db.rs](tests/db.rs)
- [tests/snapshot.rs](tests/snapshot.rs) — snapshot 功能，14 个用例，全部通过
- [tests/stress_test.rs](tests/stress_test.rs)
- [tests/stress_test_pg.rs](tests/stress_test_pg.rs)

当前已确认的事实：

- `cargo check --lib` 通过
- `cargo check --lib --no-default-features --features postgres` 通过
- `cargo test --lib` 通过
- `cargo test --lib --no-default-features --features postgres` 通过
- `cargo test --features sqlite --test snapshot` 通过（14/14）

当前仓库的已知问题：

- 集成测试没有按 feature 做门控
- 因此默认 `cargo test` 会去编译 PostgreSQL 压测文件并失败
- PostgreSQL-only 的 `cargo test --no-default-features --features postgres` 又会反过来被 SQLite 集成测试卡住

所以在测试组织修复前，不要在 README 里宣称 `cargo test` 或 `cargo test --all` 是当前可靠入口。

---

## 常见误解澄清

### “这个库已经支持多人协作”

不准确。

更准确的说法是：

- 它适合作为多人协作系统的数据核心
- 但库本身没有用户、权限、锁、审计、冲突处理

### “删除分类需要手动处理所有子分类”

不准确。

真实行为是：

- 子分类会级联删除
- 相关词条会失去 `category_id`

### “README 里的旧示例可以直接复制”

不准确。

旧版 README 曾混入这些过时写法：

- `Default::new()`
- `entry_type`
- `summary: Some(Some(...))`
- `list_entries(..., None)`

当前这份 README 已改成与现有 API 对齐的版本。

---

## 建议优先查看的文件

如果你只看 10 个文件，建议看这些：

1. [src/lib.rs](src/lib.rs)
2. [src/db/traits.rs](src/db/traits.rs)
3. [src/db/mod.rs](src/db/mod.rs)
4. [src/models/entry.rs](src/models/entry.rs)
5. [src/models/entry_type.rs](src/models/entry_type.rs)
6. [src/db/entry.rs](src/db/entry.rs)
7. [src/db/pg_entry.rs](src/db/pg_entry.rs)
8. [src/db/entry_type.rs](src/db/entry_type.rs)
9. [src/db/snapshot.rs](src/db/snapshot.rs) — SQLite 版本管理实现
10. [migrations/0001_init.sql](migrations/0001_init.sql)

---

## 版本说明

当前 crate 版本见 [Cargo.toml](Cargo.toml)。

如果后续你要继续维护 README，建议遵守两条：

- 文档示例必须先对照真实 struct 字段和 trait 签名
- 文档里的命令必须先在当前 feature 组合下实际跑过
