# Worldflow_core 核心库文档

## 🚀 快速预览

### 是什么？
Worldflow_core 是**世界观管理系统**的核心 Rust 库，提供：
- 📚 词条（Entry）：管理小说/游戏/知识库条目
- 🏢 项目（Project）：组织多个世界观
- 🗂️ 分类（Category）：树状结构分类词条
- 🏷️ 标签系统（TagSchema）：灵活的元数据标记
- ⚙️ 应用设置（AppSetting）：全局配置管理

### 核心特性
| 特性 | 说明 |
|------|------|
| **数据库** | SQLite（生产就绪）+ PostgreSQL（规划中） |
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
    let results = db.search_entries(&project.id, "主角", 10).await?;
    
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
| AppSetting | 全局配置 | key, value |

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
├── models/          # 数据模型定义
│   ├── project.rs      # 项目
│   ├── category.rs     # 分类
│   ├── entry.rs        # 词条
│   ├── tag_schema.rs   # 标签定义
│   └── app_setting.rs  # 应用设置
├── db/              # 数据库实现
│   ├── mod.rs          # SQLite 核心
│   ├── sqlite.rs       # SQLite 特定实现（预留）
│   ├── postgres.rs     # PostgreSQL 实现（规划中）
│   └── [各模块]/
│       ├── app_setting.rs
│       ├── category.rs
│       ├── entry.rs
│       ├── project.rs
│       └── tag_schema.rs
├── error.rs         # 错误定义
└── lib.rs          # 库入口
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
  └── AppSetting (项目级设置)
```

### 2.2 Project（项目）
**作用：** 顶级容器，隔离不同的世界观

```rust
pub struct Project {
    pub id: String,                    // UUID
    pub name: String,                  // 项目名称
    pub description: Option<String>,   // 描述
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
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
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
let briefs = db.list_entries(&proj_id, Some(&cat_id), 20, 0).await?;

// 全文搜索（FTS）
let results = db.search_entries(&proj_id, "战神", 10).await?;

// 统计词条数
let count = db.count_entries(&proj_id, Some(&cat_id)).await?;

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

### 2.6 AppSetting（应用设置）
**作用：** 全局配置 KV 存储

```rust
pub struct AppSetting {
    pub key: String,       // 配置键
    pub value: String,     // 配置值
    pub updated_at: String,
}
```

**内置配置项：**
| 键 | 值 | 说明 |
|----|-----|------|
| `memory_limit_mb` | 数字 or "auto" | SQLite 内存限制 |

**常用操作：**
```rust
// 读取设置
let val = db.get_setting("memory_limit_mb").await?;

// 设置值（自动 upsert）
db.set_setting("memory_limit_mb", "2048").await?;

// 删除设置
db.delete_setting("memory_limit_mb").await?;
```

---

## 三、快速开始

### 3.1 初始化项目

**依赖配置（Cargo.toml）：**
```toml
[dependencies]
worldflow_core = { path = "../worldflow_core" }
tokio = { version = "1", features = ["full"] }
sqlx = { version = "0.7", features = ["sqlite", "macros"] }
```

### 3.2 创建数据库连接
```rust
use worldflow_core::SqliteDb;

#[tokio::main]
async fn main() -> Result<()> {
    // SQLite 数据库自动创建，运行 migrations
    let db = SqliteDb::new("sqlite://data.db").await?;
    
    // 此时已自动：
    // 1. 创建所有表
    // 2. 启用外键约束
    // 3. 启用 WAL 日志
    // 4. 配置内存参数
    
    Ok(())
}
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
    let briefs = db.list_entries(&project.id, Some(&hero_cat.id), 10, 0).await?;
    println!("分类词条数: {}", briefs.len());

    let search_results = db.search_entries(&project.id, "艾琳", 10).await?;
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
```rust
// 创建
pub async fn create_entry(&self, input: CreateEntry) -> Result<Entry>;

// 获取完整词条
pub async fn get_entry(&self, id: &str) -> Result<Entry>;

// 列表（分页，返回 EntryBrief）
pub async fn list_entries(
    &self,
    project_id: &str,
    category_id: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<Vec<EntryBrief>>;

// 全文搜索
pub async fn search_entries(
    &self,
    project_id: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<EntryBrief>>;

// 统计词条数
pub async fn count_entries(
    &self,
    project_id: &str,
    category_id: Option<&str>,
) -> Result<i64>;

// 更新
pub async fn update_entry(&self, id: &str, input: UpdateEntry) -> Result<Entry>;

// 删除
pub async fn delete_entry(&self, id: &str) -> Result<()>;

// 批量创建
pub async fn create_entries_bulk(&self, inputs: Vec<CreateEntry>) -> Result<usize>;
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

### 4.5 AppSetting API
```rust
// 获取设置
pub async fn get_setting(&self, key: &str) -> Result<Option<String>>;

// 设置值（自动 upsert）
pub async fn set_setting(&self, key: &str, value: &str) -> Result<AppSetting>;

// 删除设置
pub async fn delete_setting(&self, key: &str) -> Result<()>;
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
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

**categories**
```sql
CREATE TABLE categories (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id),
    parent_id TEXT REFERENCES categories(id),
    name TEXT NOT NULL,
    sort_order INTEGER DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    
    UNIQUE(project_id, parent_id, name)
);
```

**entries**
```sql
CREATE TABLE entries (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id),
    category_id TEXT REFERENCES categories(id),
    title TEXT NOT NULL,
    summary TEXT,
    content TEXT NOT NULL,
    type TEXT,
    tags TEXT,          -- JSON
    images TEXT,        -- JSON
    cover_path TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- FTS 虚拟表用于全文搜索
CREATE VIRTUAL TABLE entries_fts USING fts5(
    title,
    summary,
    content,
    content = 'entries',
    content_rowid = 'rowid'
);
```

**tag_schemas**
```sql
CREATE TABLE tag_schemas (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id),
    name TEXT NOT NULL,
    description TEXT,
    type TEXT NOT NULL,  -- "number", "string", "boolean"
    target TEXT NOT NULL,  -- JSON array
    default_val TEXT,
    range_min REAL,
    range_max REAL,
    sort_order INTEGER DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    
    UNIQUE(project_id, name)
);
```

**app_settings**
```sql
CREATE TABLE app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### 5.2 迁移文件位置
```
migrations/
├── 01_init.sql      -- 初始表创建
├── 02_fts.sql       -- FTS 虚拟表
└── ...
```

迁移通过 `sqlx migrate!()` 宏自动运行。

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

**自定义内存限制：**
通过 `AppSetting` 覆盖自动检测：
```rust
db.set_setting("memory_limit_mb", "4096").await?;
```

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
- ✅ FTS5 虚拟表
- ✅ 支持布尔查询（AND, OR, NOT）
- ✅ 示例：`db.search_entries(proj, "dragon AND fire", 20).await?`

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
**A:** FTS 查询速度很快（通常 <100ms），但首次启用 FTS 需构建索引。建议在初始化时预热。

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

### 9.1 PostgreSQL 支持（规划中）

**预留结构：**
```
db/postgres.rs  (当前为空，待实现)
```

**实现步骤：**
1. 在 `postgres.rs` 中实现 `PostgresDb` 结构
2. 为各模块实现 PostgreSQL 版本
3. 添加功能开关：`#[cfg(feature = "postgres")]`
4. 更新 `Cargo.toml` 依赖

### 9.2 新增功能建议

**潜在扩展：**
- [ ] 版本历史（词条变更记录）
- [ ] 权限系统（分享、协作权限）
- [ ] 关系图（词条间的链接）
- [ ] 批量操作 API
- [ ] WebAssembly 支持

### 9.3 代码贡献规范

**目录结构遵循：**
```
新增模块 X
├── models/x.rs          -- 数据模型
├── db/x.rs              -- SQLite 实现
├── db/postgres/x.rs     -- PostgreSQL 实现（预留）
└── 更新 models/mod.rs 和 db/mod.rs
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
| **AppSetting** | 全局配置 | AppSetting        |

**关键设计决策：**
- ✅ SQLite + WAL：轻量、适合嵌入式
- ✅ JSON 存储：灵活，支持自定义数据
- ✅ FTS5：高性能全文搜索
- ✅ 自适应内存：自动优化性能
- ✅ 树状分类：无限层级 + 循环检测

---

**文档版本：** 1.0  
**最后更新：** 2026-03-23  
**维护者：** Worldflow 开发团队