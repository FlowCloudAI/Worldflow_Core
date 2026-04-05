# Worldflow_core 核心库文档

## 🚀 快速预览

### 是什么？
Worldflow_core 是**世界观管理系统**的核心 Rust 库，提供：
- 📚 词条（Entry）：管理小说/游戏/知识库条目
- 🏢 项目（Project）：组织多个世界观
- 🗂️ 分类（Category）：树状结构分类词条
- 🏷️ 标签系统（TagSchema）：灵活的元数据标记
- 🔗 词条关系（EntryRelation）：词条间的关联和网络

### 核心特性
| 特性 | 说明 |
|------|------|
| **数据库** | SQLite（生产就绪）+ PostgreSQL（已实现，feature flag 控制） |
| **性能** | 自适应内存管理、FTS全文搜索、WAL 日志 |
| **安全** | 外键约束、循环检测、事务支持 |
| **灵活** | JSON 存储、树状分类、自定义标签 |

### 五分钟上手
```rust
use worldflow_core::SqliteDb;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. 初始化数据库
    let db = SqliteDb::new("sqlite://data.db").await?;

    // 2. 创建项目
    let project = db.create_project(CreateProject {
        name: "我的世界观".to_string(),
        description: Some("一个奇幻世界".to_string()),
    }).await?;

    // 3. 添加词条
    let entry = db.create_entry(CreateEntry {
        project_id: project.id.clone(),
        title: "主角".to_string(),
        category_id: None,
        summary: Some("故事的主人公".to_string()),
        content: Some("详细描述...".to_string()),
        ..Default::new()
    }).await?;

    // 4. 搜索词条
    let results = db.search_entries(&project.id, "主角", EntryFilter::default(), 10).await?;
    
    Ok(())
}
```

### 核心概念速查表
| 类型 | 作用 | 关键字段 |
|------|------|---------|
| Project | 顶级容器 | id, name, description |
| Category | 分类（树状） | parent_id, sort_order |
| Entry | 词条内容 | title, content, tags, images |
| TagSchema | 标签定义 | type(number/string/boolean), target |
| EntryRelation | 词条关系 | a_id, b_id, relation(one_way/two_way) |

---

# 📖 完整文档

## 一、项目简介

### 1.1 背景与目标
Worldflow_core 是为内容创作者（小说、游戏、知识库）设计的**世界观管理系统**核心库。

**核心目标：**
- 灵活存储复杂的内容关系（词条、分类、标签）
- 高性能查询和全文搜索
- 支持多项目、多用户场景
- 为上层应用提供统一的数据访问接口

### 1.2 主要模块
```
worldflow_core/
├── models/                 # 数据模型定义
│   ├── project.rs
│   ├── category.rs
│   ├── entry.rs
│   ├── tag_schema.rs
│   └── entry_relation.rs
├── db/                     # 数据库实现
│   ├── mod.rs              # SqliteDb / PgDb 结构体 + 初始化
│   ├── traits.rs           # Db trait 定义（ProjectOps 等）
│   ├── project.rs          # SQLite 实现
│   ├── category.rs
│   ├── entry.rs
│   ├── tag_schema.rs
│   ├── entry_relation.rs
│   ├── pg_project.rs       # PostgreSQL 实现
│   ├── pg_category.rs
│   ├── pg_entry.rs
│   ├── pg_tag_schema.rs
│   └── pg_entry_relation.rs
├── error.rs                # 错误定义
└── lib.rs                  # 库入口 + trait 重导出
```

---

## 二、核心概念与数据模型

### 2.1 数据关系图
```
Project (项目)
  ├── Category (分类树)
  │   └── parent_id → parent Category
  ├── Entry (词条)
  │   ├── category_id → Category
  │   ├── tags[] → TagSchema
  │   └── images[]
  ├── TagSchema (标签定义)
  │   └── target: ["character", "item", ...]
  └── EntryRelation (词条关系)
      ├── a_id → Entry
      └── b_id → Entry
```

### 2.2 Project（项目）
**作用：** 顶级容器，隔离不同的世界观

```rust
pub struct Project {
    pub id: String,                    // UUID
    pub name: String,                  // 项目名称
    pub description: Option<String>,   // 描述
    pub cover_path: Option<String>,    // 项目封面路径
    pub created_at: String,            // 创建时间
    pub updated_at: String,            // 更新时间
}
```

**数据库表：**
```sql
CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    cover_path TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

**常用操作：**
```rust
// 创建
db.create_project(CreateProject { name, description }).await?;

// 查询
db.get_project(id).await?;
db.list_projects().await?;

// 更新
db.update_project(id, UpdateProject { name, description }).await?;

// 删除
db.delete_project(id).await?;
```

### 2.3 Category（分类树）
**作用：** 词条的分类系统，支持多层树状结构（无限深度）

```rust
pub struct Category {
    pub id: String,               // UUID
    pub project_id: String,       // 所属项目
    pub parent_id: Option<String>,// 父级分类（None = 根节点）
    pub name: String,             // 分类名称
    pub sort_order: i64,          // 排序（同级）
    pub created_at: String,
    pub updated_at: String,
}
```

**关键特性：**
- ✅ 无限层级：支持任意深度嵌套
- ✅ 循环检测：禁止将分类移到其子孙节点下
- ✅ 树状排序：`parent_id NULLS FIRST, sort_order`

**常用操作：**
```rust
// 创建根分类
db.create_category(CreateCategory {
    project_id: proj_id,
    parent_id: None,  // 根节点
    name: "人物".to_string(),
    sort_order: Some(1),
}).await?;

// 创建子分类
db.create_category(CreateCategory {
    project_id: proj_id,
    parent_id: Some(parent_cat_id),
    name: "主角".to_string(),
    sort_order: Some(1),
}).await?;

// 列出某项目的所有分类（树序）
let categories = db.list_categories(&proj_id).await?;

// 移动分类（自动检测循环）
db.update_category(cat_id, UpdateCategory {
    parent_id: Some(Some(new_parent_id)),  // Some(Some(...)) = 设置新父节点
    ..Default::new()
}).await?;

// 移到根节点
db.update_category(cat_id, UpdateCategory {
    parent_id: Some(None),  // Some(None) = 移到根
    ..Default::new()
}).await?;
```

**循环检测原理：**
```sql
-- 检查 cat_id 的所有子孙中是否包含 new_parent_id
WITH RECURSIVE descendants(id) AS (
    SELECT id FROM categories WHERE id = ?
    UNION ALL
    SELECT c.id FROM categories c
    JOIN descendants d ON c.parent_id = d.id
)
SELECT COUNT(*) FROM descendants WHERE id = ?
```

### 2.4 Entry（词条）
**作用：** 核心内容单元，存储实际的世界观信息

```rust
pub struct Entry {
    pub id: String,
    pub project_id: String,
    pub category_id: Option<String>,  // 可选归类
    pub title: String,                // 词条标题
    pub summary: Option<String>,      // 摘要（新增）
    pub content: String,              // 完整内容
    pub r#type: Option<String>,       // 内容类型
    pub tags: Json<Vec<EntryTag>>,    // 标签数组
    pub images: Json<Vec<FCImage>>,   // 图像数组
    pub cover_path: Option<String>,   // 封面（自动计算）
    pub created_at: String,
    pub updated_at: String,
}

// 标签项
pub struct EntryTag {
    pub schema_id: String,            // 对应的 TagSchema.id
    pub value: serde_json::Value,     // 标签值（任意 JSON）
}

// 图像信息
pub struct FCImage {
    pub path: PathBuf,
    pub is_cover: bool,               // 是否为封面
    pub caption: Option<String>,      // 图注
}
```

**列表用轻量版：**
```rust
pub struct EntryBrief {
    // 省略 content 和完整 tags，减少反序列化
    pub id: String,
    pub title: String,
    pub summary: Option<String>,
    pub cover: Option<PathBuf>,
    pub updated_at: String,
}
```

**常用操作：**
```rust
// 创建词条
db.create_entry(CreateEntry {
    project_id: proj_id,
    category_id: Some(cat_id),
    title: "阿瑞斯".to_string(),
    summary: Some("战神".to_string()),
    content: Some("强大的神灵...".to_string()),
    tags: Some(vec![
        EntryTag {
            schema_id: tag_schema_id,
            value: json!(80),
        }
    ]),
    images: Some(vec![
        FCImage {
            path: "image.jpg".into(),
            is_cover: true,
            caption: Some("官方海报".to_string()),
        }
    ]),
    ..Default::new()
}).await?;

// 获取完整词条（含所有数据）
let entry = db.get_entry(&entry_id).await?;

// 列表查询（分页，不含 content）
let briefs = db.list_entries(&proj_id, EntryFilter { category_id: Some(&cat_id), ..Default::default() }, 20, 0).await?;

// 全文搜索（FTS）
let results = db.search_entries(&proj_id, "战神", EntryFilter::default(), 10).await?;

// 统计词条数
let count = db.count_entries(&proj_id, EntryFilter { category_id: Some(&cat_id), ..Default::default() }).await?;

// 批量创建
db.create_entries_bulk(entries).await?;
```

**性能优化：**
- `EntryBrief`：列表操作省略 `content` 和完整标签，减少反序列化开销
- **FTS 全文搜索**：基于 SQLite 虚拟表 `entries_fts`
- **分页**：支持 LIMIT/OFFSET

### 2.5 TagSchema（标签定义）
**作用：** 定义可应用于词条的标签类型和约束

```rust
pub struct TagSchema {
    pub id: String,
    pub project_id: String,
    pub name: String,                 // 标签名称
    pub description: Option<String>,
    pub r#type: String,               // "number" | "string" | "boolean"
    pub target: String,               // JSON: ["character", "item", ...]
    pub default_val: Option<String>,  // 默认值
    pub range_min: Option<f64>,       // 数值范围（仅 type=number）
    pub range_max: Option<f64>,
    pub sort_order: i64,              // 排序
    pub created_at: String,
    pub updated_at: String,
}
```

**约束校验：**
- `type=number`：default_val 必须能解析为 f64
- `type=boolean`：default_val 只能是 "true" 或 "false"
- `range_min/max`：仅对数值类型有效

**常用操作：**
```rust
// 创建标签定义
db.create_tag_schema(CreateTagSchema {
    project_id: proj_id,
    name: "能力值".to_string(),
    description: Some("角色基础属性".to_string()),
    r#type: "number".to_string(),
    target: vec!["character".to_string()],  // 仅可标记角色
    default_val: Some("50".to_string()),
    range_min: Some(0.0),
    range_max: Some(100.0),
    sort_order: Some(1),
}).await?;

// 查询标签定义
db.get_tag_schema(&schema_id).await?;
db.list_tag_schemas(&proj_id).await?;

// 更新标签定义
db.update_tag_schema(&schema_id, CreateTagSchema { ... }).await?;

// 删除标签定义
db.delete_tag_schema(&schema_id).await?;
```

### 2.6 EntryRelation（词条关系）
**作用：** 建立词条间的关联关系（如人物关系、情节线索等）

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationDirection {
    OneWay,  // 单向关系 (A → B)
    TwoWay,  // 双向关系 (A ↔ B)
}

pub struct EntryRelation {
    pub id:         String,              // UUID
    pub project_id: String,              // 所属项目
    pub a_id:       String,              // 词条A（单向时为源；双向时总是 < b_id）
    pub b_id:       String,              // 词条B（单向时为目标；双向时总是 > a_id）
    pub relation:   RelationDirection,   // 关系方向
    pub content:    String,              // 关系描述
    pub created_at: String,
    pub updated_at: String,
}
```

**关键特性：**
- ✅ 单向/双向关系：灵活描述不同关系类型
- ✅ 关系内容：自由文本描述关系的具体内容
- ✅ 跨项目检查：确保关联的词条属于同一项目
- ✅ 双向关系规范化：two_way 关系自动规范化为 `a_id < b_id`，消除重复

**常用操作：**
```rust
// 创建关系
db.create_relation(CreateEntryRelation {
    project_id: proj_id,
    a_id: char1_id,
    b_id: char2_id,
    relation: RelationDirection::TwoWay,
    content: "相互爱恋的恋人".to_string(),
}).await?;

// 查询词条的所有关系
let relations = db.list_relations_for_entry(&entry_id).await?;

// 查询项目的所有关系（构建关系图）
let all_relations = db.list_relations_for_project(&proj_id).await?;

// 更新关系
db.update_relation(&relation_id, UpdateEntryRelation {
    relation: Some(RelationDirection::OneWay),
    content: Some("单向喜欢".to_string()),
}).await?;

// 删除关系
db.delete_relation(&relation_id).await?;

// 删除两个词条间的所有关系
db.delete_relations_between(&entry_a_id, &entry_b_id).await?;
```

---

## 三、快速开始

### 3.1 初始化项目

**依赖配置（Cargo.toml）：**
```toml
[dependencies]
worldflow_core = { path = "../worldflow_core" }            # 默认 SQLite
# worldflow_core = { path = "../worldflow_core", features = ["postgres"] }  # 启用 PostgreSQL
tokio = { version = "1", features = ["full"] }
```

### 3.2 创建数据库连接
```rust
// SQLite
use worldflow_core::{SqliteDb, ProjectOps, EntryOps}; // 按需导入 trait

let db = SqliteDb::new("sqlite://data.db").await?;
// 自动：建表、启用外键、WAL 日志、自适应内存配置

// PostgreSQL（需启用 feature = "postgres"）
use worldflow_core::PgDb;

let db = PgDb::new("postgres://user:pass@localhost/dbname").await?;
```

**注：** 方法通过 trait 提供，使用前需将对应 trait 引入作用域：
```rust
use worldflow_core::{ProjectOps, CategoryOps, EntryOps, TagSchemaOps, EntryRelationOps};
// 或一次性导入组合 trait：
use worldflow_core::Db;
```

### 3.3 完整示例：创建世界观

```rust
use worldflow_core::{SqliteDb, models::*};

#[tokio::main]
async fn main() -> Result<()> {
    let db = SqliteDb::new("sqlite://fantasy.db").await?;

    // 1️⃣ 创建项目
    let project = db.create_project(CreateProject {
        name: "龙与火焰".to_string(),
        description: Some("一个奇幻世界".to_string()),
    }).await?;
    println!("项目创建: {:?}", project.id);

    // 2️⃣ 创建分类结构
    let role_cat = db.create_category(CreateCategory {
        project_id: project.id.clone(),
        parent_id: None,
        name: "角色".to_string(),
        sort_order: Some(1),
    }).await?;

    let hero_cat = db.create_category(CreateCategory {
        project_id: project.id.clone(),
        parent_id: Some(role_cat.id.clone()),
        name: "主角".to_string(),
        sort_order: Some(1),
    }).await?;

    // 3️⃣ 创建标签定义
    let strength_tag = db.create_tag_schema(CreateTagSchema {
        project_id: project.id.clone(),
        name: "力量".to_string(),
        r#type: "number".to_string(),
        target: vec!["character".to_string()],
        default_val: Some("50".to_string()),
        range_min: Some(0.0),
        range_max: Some(100.0),
        ..Default::new()
    }).await?;

    // 4️⃣ 创建词条（带标签和图像）
    let entry = db.create_entry(CreateEntry {
        project_id: project.id.clone(),
        category_id: Some(hero_cat.id.clone()),
        title: "艾琳".to_string(),
        summary: Some("故事的主人公".to_string()),
        content: Some("艾琳是一位年轻的骑士...".to_string()),
        r#type: Some("character".to_string()),
        tags: Some(vec![
            EntryTag {
                schema_id: strength_tag.id,
                value: serde_json::json!(85),
            }
        ]),
        images: Some(vec![
            FCImage {
                path: "erin.jpg".into(),
                is_cover: true,
                caption: Some("官方设定".to_string()),
            }
        ]),
    }).await?;
    println!("词条创建: {}", entry.title);

    // 5️⃣ 查询与搜索
    let briefs = db.list_entries(&project.id, EntryFilter { category_id: Some(&hero_cat.id), ..Default::default() }, 10, 0).await?;
    println!("分类词条数: {}", briefs.len());

    let search_results = db.search_entries(&project.id, "艾琳", EntryFilter::default(), 10).await?;
    println!("搜索结果: {}", search_results.len());

    // 6️⃣ 更新词条
    let updated = db.update_entry(&entry.id, UpdateEntry {
        summary: Some(Some("勇敢的骑士".to_string())),
        ..Default::new()
    }).await?;
    println!("词条更新: {}", updated.summary.unwrap_or_default());

    Ok(())
}
```

---

## 四、API 文档

### 4.1 Project API
```rust
// 创建
pub async fn create_project(&self, input: CreateProject) -> Result<Project>;

// 查询单个
pub async fn get_project(&self, id: &str) -> Result<Project>;

// 列表
pub async fn list_projects(&self) -> Result<Vec<Project>>;

// 更新
pub async fn update_project(&self, id: &str, input: UpdateProject) -> Result<Project>;

// 删除
pub async fn delete_project(&self, id: &str) -> Result<()>;
```

### 4.2 Category API
```rust
// 创建
pub async fn create_category(&self, input: CreateCategory) -> Result<Category>;

// 查询单个
pub async fn get_category(&self, id: &str) -> Result<Category>;

// 列表（全部，按树序排列）
pub async fn list_categories(&self, project_id: &str) -> Result<Vec<Category>>;

// 更新（支持移动）
pub async fn update_category(&self, id: &str, input: UpdateCategory) -> Result<Category>;

// 删除
pub async fn delete_category(&self, id: &str) -> Result<()>;

// 检查是否会形成循环
pub async fn would_create_cycle(&self, id: &str, new_parent_id: &str) -> Result<bool>;
```

### 4.3 Entry API

**EntryFilter 结构体（支持灵活的多条件过滤）：**
```rust
pub struct EntryFilter<'a> {
    pub category_id: Option<&'a str>,  // 按分类筛选
    pub entry_type: Option<&'a str>,   // 按词条类型筛选（如 "character", "item" 等）
}

// 实现了 Default，方便无条件查询
impl Default for EntryFilter<'_> {
    fn default() -> Self {
        EntryFilter { category_id: None, entry_type: None }
    }
}
```

**API 方法：**
```rust
// 创建
pub async fn create_entry(&self, input: CreateEntry) -> Result<Entry>;

// 获取完整词条
pub async fn get_entry(&self, id: &str) -> Result<Entry>;

// 列表（分页，返回 EntryBrief，支持按分类和类型过滤）
pub async fn list_entries(
    &self,
    project_id: &str,
    filter: EntryFilter<'_>,
    limit: usize,
    offset: usize,
) -> Result<Vec<EntryBrief>>;

// 全文搜索（支持按分类和类型过滤）
pub async fn search_entries(
    &self,
    project_id: &str,
    query: &str,
    filter: EntryFilter<'_>,
    limit: usize,
) -> Result<Vec<EntryBrief>>;

// 统计词条数（支持按分类和类型过滤）
pub async fn count_entries(
    &self,
    project_id: &str,
    filter: EntryFilter<'_>,
) -> Result<i64>;

// 更新
pub async fn update_entry(&self, id: &str, input: UpdateEntry) -> Result<Entry>;

// 删除
pub async fn delete_entry(&self, id: &str) -> Result<()>;

// 批量创建
pub async fn create_entries_bulk(&self, inputs: Vec<CreateEntry>) -> Result<usize>;
```

**EntryFilter 使用示例：**
```rust
use worldflow_core::EntryFilter;

// 示例 1：列出项目的所有词条（无过滤）
let all = db.list_entries(&proj_id, EntryFilter::default(), 20, 0).await?;

// 示例 2：列出特定分类下的所有词条
let by_cat = db.list_entries(
    &proj_id,
    EntryFilter { category_id: Some(&cat_id), ..Default::default() },
    20,
    0
).await?;

// 示例 3：列出项目中所有 "character" 类型的词条
let by_type = db.list_entries(
    &proj_id,
    EntryFilter { entry_type: Some("character"), ..Default::default() },
    20,
    0
).await?;

// 示例 4：列出特定分类且特定类型的词条
let by_cat_and_type = db.list_entries(
    &proj_id,
    EntryFilter {
        category_id: Some(&cat_id),
        entry_type: Some("character"),
    },
    20,
    0
).await?;

// 示例 5：搜索 "龙" 字，但只搜索 "event" 类型的词条
let search_events = db.search_entries(
    &proj_id,
    "龙",
    EntryFilter { entry_type: Some("event"), ..Default::default() },
    10
).await?;

// 示例 6：统计某分类下的词条数
let count = db.count_entries(
    &proj_id,
    EntryFilter { category_id: Some(&cat_id), ..Default::default() }
).await?;
```

### 4.6 FTS 维护 API（仅 SQLite）
```rust
// 批量写入后调用，消除 FTS 碎片，恢复搜索性能
pub async fn optimize_fts(&self) -> Result<()>;
```

**使用时机：** 每次 `create_entries_bulk` 之后调用一次即可：
```rust
db.create_entries_bulk(entries).await?;
db.optimize_fts().await?;
```

### 4.4 TagSchema API
```rust
// 创建
pub async fn create_tag_schema(&self, input: CreateTagSchema) -> Result<TagSchema>;

// 查询
pub async fn get_tag_schema(&self, id: &str) -> Result<TagSchema>;

// 列表
pub async fn list_tag_schemas(&self, project_id: &str) -> Result<Vec<TagSchema>>;

// 更新
pub async fn update_tag_schema(&self, id: &str, input: CreateTagSchema) -> Result<TagSchema>;

// 删除
pub async fn delete_tag_schema(&self, id: &str) -> Result<()>;
```

### 4.5 EntryRelation API
```rust
// 创建词条关系
pub async fn create_relation(&self, input: CreateEntryRelation) -> Result<EntryRelation>;

// 查询单个关系
pub async fn get_relation(&self, id: &str) -> Result<EntryRelation>;

// 查询词条的所有关系（含双向）
pub async fn list_relations_for_entry(&self, entry_id: &str) -> Result<Vec<EntryRelation>>;

// 查询项目的所有关系（用于关系图）
pub async fn list_relations_for_project(&self, project_id: &str) -> Result<Vec<EntryRelation>>;

// 更新关系
pub async fn update_relation(&self, id: &str, input: UpdateEntryRelation) -> Result<EntryRelation>;

// 删除单个关系
pub async fn delete_relation(&self, id: &str) -> Result<()>;

// 删除两个词条间的所有关系
pub async fn delete_relations_between(&self, entry_a: &str, entry_b: &str) -> Result<u64>;
```

---

## 五、数据库设计与迁移

### 5.1 表结构

**projects**
```sql
CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    cover_path TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

**categories**
```sql
CREATE TABLE categories (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    parent_id TEXT,
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    CHECK (id != parent_id),
    UNIQUE (project_id, id),
    FOREIGN KEY (project_id, parent_id) REFERENCES categories(project_id, id) ON DELETE CASCADE
);
```

**entries**
```sql
CREATE TABLE entries (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    category_id TEXT REFERENCES categories(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    summary TEXT,
    content TEXT NOT NULL DEFAULT '',
    type TEXT,
    tags TEXT NOT NULL DEFAULT '[]',          -- JSON
    images TEXT NOT NULL DEFAULT '[]',        -- JSON
    cover_path TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

**tag_schemas**
```sql
CREATE TABLE tag_schemas (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    type TEXT NOT NULL CHECK(type IN ('number', 'string', 'boolean')),
    target TEXT NOT NULL DEFAULT '[]',  -- JSON array
    default_val TEXT,
    range_min REAL,
    range_max REAL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    CHECK (range_min IS NULL OR range_max IS NULL OR range_min <= range_max)
);
```

**entry_relations**
```sql
CREATE TABLE entry_relations (
    id         TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    a_id       TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    b_id       TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    relation   TEXT NOT NULL CHECK(relation IN ('one_way', 'two_way')),
    content    TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (a_id, b_id, content)
);
```

### 5.2 迁移文件位置
```
migrations/
└── 0001_init.sql    -- 初始表创建和触发器
```

迁移通过 `sqlx migrate!()` 宏自动运行，包含所有表创建和触发器定义。

---

## 六、性能优化

### 6.1 内存管理（SQLite）

**自适应策略：**
系统根据可用内存自动配置：

```rust
// 初始化时自动检测
let available_mb = sys.available_memory() / 1024 / 1024;

match available_mb {
    > 16000 => {
        cache_size = -131072KB  (128MB)
        mmap_size = 1GB
        temp_store = MEMORY
    }
    > 8000  => {
        cache_size = -65536KB   (64MB)
        mmap_size = 512MB
        temp_store = MEMORY
    }
    > 4000  => {
        cache_size = -32768KB   (32MB)
        mmap_size = 256MB
        temp_store = MEMORY
    }
    else    => {
        cache_size = -16384KB   (16MB)
        mmap_size = 128MB
        temp_store = DEFAULT
    }
}
```

**注：** 内存限制在初始化时通过自动检测设置，当前版本不支持运行时动态调整。

### 6.2 查询优化

**索引策略：**
```sql
-- 必要索引（自动创建）
CREATE INDEX idx_entries_project_id ON entries(project_id);
CREATE INDEX idx_entries_category_id ON entries(category_id);
CREATE INDEX idx_categories_project_id ON categories(project_id);
CREATE INDEX idx_tag_schemas_project_id ON tag_schemas(project_id);
```

**列表查询优化：**
- ✅ 使用 `EntryBrief` 避免反序列化大量数据
- ✅ 分页查询使用 LIMIT/OFFSET
- ✅ 预计算 `cover_path` 字段

**全文搜索优化：**
- ✅ FTS5 虚拟表（SQLite）/ GIN 索引 + tsvector（PostgreSQL）
- ✅ 支持布尔查询（AND, OR, NOT）
- ✅ 示例：`db.search_entries(proj, "dragon AND fire", 20).await?`
- ✅ 批量写入后调用 `optimize_fts()` 消除碎片（SQLite）

### 6.3 并发控制

**连接池配置：**
```rust
SqlitePoolOptions::new()
    .max_connections(5)  // SQLite 推荐值
    .connect(database_url)
    .await?
```

**事务支持：**
```rust
let mut tx = db.pool.begin().await?;
// ... 多个操作 ...
tx.commit().await?;
```

---

## 七、错误处理

### 7.1 错误类型

```rust
pub enum WorldflowError {
    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),

    #[error("迁移错误: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("记录不存在: {0}")]
    NotFound(String),  // 如 "category 123"

    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("参数错误: {0}")]
    InvalidInput(String),  // 如循环分类、非法数值范围
}

pub type Result<T> = std::result::Result<T, WorldflowError>;
```

### 7.2 常见错误处理
```rust
use worldflow_core::error::WorldflowError;

match db.get_entry(&id).await {
    Ok(entry) => { /* 处理 */ },
    Err(WorldflowError::NotFound(msg)) => {
        eprintln!("未找到: {}", msg);
    },
    Err(WorldflowError::InvalidInput(msg)) => {
        eprintln!("输入错误: {}", msg);
    },
    Err(e) => {
        eprintln!("其他错误: {}", e);
    }
}
```

---

## 八、常见问题

### Q1: 如何实现"多人协作编辑"？
**A:** 使用 `updated_at` 字段实现乐观锁：
```rust
// 检查是否已被修改
let current = db.get_entry(&id).await?;
if current.updated_at != original_updated_at {
    return Err(WorldflowError::InvalidInput("数据已被修改".to_string()));
}
// 继续更新...
```

### Q2: 如何处理删除分类时的子分类？
**A:** 暂无自动级联，需要手动处理：
```rust
// 1. 查出所有子分类
let children = db.list_categories(&project_id).await?
    .into_iter()
    .filter(|c| c.parent_id == Some(cat_id.to_string()))
    .collect::<Vec<_>>();

// 2. 选择操作：删除 / 移动 / 不删除
for child in children {
    db.update_category(&child.id, UpdateCategory {
        parent_id: Some(None),  // 移到根
        ..Default::new()
    }).await?;
}

// 3. 删除分类
db.delete_category(&cat_id).await?;
```

### Q3: 搜索的性能如何？
**A:** FTS 查询速度很快（通常 <100ms）。大量写入后搜索可能变慢（FTS 碎片化），调用 `optimize_fts()` 即可恢复：
```rust
db.create_entries_bulk(data).await?;
db.optimize_fts().await?;  // 批量写入后调用
```

### Q4: 支持什么类型的图像？
**A:** 系统仅存储路径，不验证格式。支持：
- 本地路径：`/data/images/hero.jpg`
- URL：`https://example.com/image.png`
- 相对路径：`images/hero.jpg`

### Q5: 如何备份数据库？
**A:** SQLite 备份很简单：
```bash
# 关闭应用后
cp data.db data.db.backup
```

---

## 九、扩展与贡献

### 9.1 PostgreSQL 支持

**已实现**，通过 feature flag 启用：
```toml
worldflow_core = { path = "...", features = ["postgres"] }
```

**架构：** 所有操作通过 trait 定义，SQLite 和 PostgreSQL 分别实现：
```rust
// 泛型函数可同时支持两种后端
async fn do_work(db: &impl Db) {
    db.create_project(...).await?;
}

// 或具体类型
let sqlite_db = SqliteDb::new("sqlite://data.db").await?;
let pg_db     = PgDb::new("postgres://...").await?;
```

**主要差异：**
| | SQLite | PostgreSQL |
|---|---|---|
| 参数占位符 | `?` | `$1, $2, ...` |
| 全文搜索 | FTS5 虚拟表 | GIN + tsvector |
| 时间类型 | TEXT | TIMESTAMPTZ（查询时转 TEXT） |
| 连接数 | max 5 | max 10 |
| 迁移目录 | `migrations/` | `migrations_pg/` |

### 9.2 新增功能建议

**潜在扩展：**
- [ ] 版本历史（词条变更记录）
- [ ] 权限系统（分享、协作权限）
- [ ] 批量操作 API 扩展
- [ ] WebAssembly 支持

### 9.3 代码贡献规范

**新增模块 X 的目录结构：**
```
models/x.rs          -- 数据模型
db/traits.rs         -- 在对应 trait 中添加方法
db/x.rs              -- impl XxxOps for SqliteDb
db/pg_x.rs           -- impl XxxOps for PgDb
更新 models/mod.rs 和 db/mod.rs
```

**测试建议：**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_xxx() {
        let db = SqliteDb::new("sqlite://:memory:").await.unwrap();
        // 测试代码...
    }
}
```

---

## 十、FAQ - 开发维护

### 如何本地开发？

**环境要求：**
- Rust 1.70+
- SQLx CLI（用于迁移）

**本地运行：**
```bash
# 安装 SQLx CLI
cargo install sqlx-cli

# 创建本地数据库
sqlx database create

# 运行迁移
sqlx migrate run

# 运行测试
cargo test --all

# 查看文档
cargo doc --open
```

### 版本管理策略

遵循 Semantic Versioning：
- **主版本**：API 不兼容变更
- **次版本**：新功能（向后兼容）
- **补丁版本**：bug 修复

---

## 总结

| 模块             | 职责   | 核心类型              |
|----------------|------|-------------------|
| **Project**    | 项目容器 | Project           |
| **Category**   | 分类树  | Category（支持循环检测）  |
| **Entry**      | 词条内容 | Entry, EntryBrief |
| **TagSchema**  | 标签定义 | TagSchema（支持类型约束） |
| **EntryRelation** | 词条关系 | EntryRelation（单向/双向） |

**关键设计决策：**
- ✅ Trait 架构：`ProjectOps` 等子 trait + `Db` 组合 trait，SQLite/PG 均实现
- ✅ SQLite + WAL：轻量、适合嵌入式
- ✅ PostgreSQL：feature flag 启用，适合多用户/云部署
- ✅ JSON 存储：灵活，支持自定义数据
- ✅ FTS5 / GIN：高性能全文搜索，`optimize_fts()` 消除碎片
- ✅ 自适应内存：SQLite 自动优化性能
- ✅ 树状分类：无限层级 + 循环检测

---

**文档版本：** 1.3
**最后更新：** 2026-04-04
**维护者：** Worldflow 开发团队

---

## 更新日志

### v1.3 (2026-04-04)
- ✨ 新增：`EntryFilter` 结构体支持灵活多条件过滤（按分类、词条类型）
- 📝 更新：`list_entries`、`search_entries`、`count_entries` 等方法增加 `EntryFilter` 参数
- 📚 更新：API 文档和示例，展示 `EntryFilter` 的各种使用场景
- ✅ 改进：支持同时按分类和类型过滤词条

### v1.2 (2026-04-02)
- ✨ 新增：PostgreSQL 支持（`features = ["postgres"]`，`PgDb` 结构体）
- ✨ 新增：Trait 架构（`ProjectOps`/`CategoryOps`/`EntryOps`/`TagSchemaOps`/`EntryRelationOps`/`Db`）
- ✨ 新增：`optimize_fts()` 方法，消除批量写入后的 FTS 碎片
- 📚 更新：lib.rs 重导出所有 trait，使用方无需手动导入路径

### v1.1 (2026-04-02)
- ✨ 新增：EntryRelation 模块支持词条间的关系管理（单向/双向）
- 🗑️ 移除：AppSetting 全局配置模块
- 📦 整合：将 FTS 全文搜索与初始化合并到单一迁移文件

### v1.0 (2026-03-23)
- 初始版本发布
- 核心模块：Project、Category、Entry、TagSchema
- SQLite 支持和性能优化