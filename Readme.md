# core_world_data

`core_world_data`（Rust crate 名：`worldflow_core`）是 FlowCloudAI 的世界观数据存储库，提供项目、分类、词条、标签、关系、内部链接、想法笔记、API 用量和快照版本管理等核心数据能力。它用统一 trait 屏蔽 SQLite / PostgreSQL 差异，供桌面端和后续服务端复用。

## 快速开始

### 环境要求

- Rust stable，Edition 2024。
- 默认 feature 为 `sqlite` + `snapshot`。
- PostgreSQL 需要显式关闭默认 feature 并开启 `postgres`。

### 构建与测试

```bash
cd core_world_data

# 默认 SQLite + snapshot
cargo check --lib
cargo test --lib

# PostgreSQL 编译检查
cargo check --lib --no-default-features --features postgres

# PostgreSQL 门控测试；需要可用 DATABASE_URL 或本地 PostgreSQL
cargo test --test stress_test_pg --no-default-features --features postgres

# 运行全部可用测试
cargo test
```

### 最小 SQLite 示例

```rust
use worldflow_core::{ProjectOps, SqliteDb, models::CreateProject};

#[tokio::main]
async fn main() -> worldflow_core::Result<()> {
    let db = SqliteDb::new("sqlite:worldflow.db?mode=rwc").await?;

    let project = db
        .create_project(CreateProject {
            name: "示例世界".to_string(),
            description: Some("用于验证 worldflow_core 的最小项目".to_string()),
            cover_image: None,
        })
        .await?;

    println!("创建项目：{} / {}", project.id, project.name);
    Ok(())
}
```

## 主要功能 / 使用方式

- **项目与分类**：`ProjectOps`、`CategoryOps` 支持项目 CRUD、分类树、批量创建和循环检测。
- **词条与搜索**：`EntryOps` 支持词条分页、过滤、全文搜索、批量创建和摘要字段更新。
- **词条内部链接**：`EntryLinkOps` 管理词条间的正文链接关系。
- **标签与类型**：`TagSchemaOps`、`EntryTypeOps` 管理标签结构和自定义词条类型。
- **关系图**：`EntryRelationOps` 管理词条关系边，支持项目级和词条级查询。
- **想法笔记**：`IdeaNoteOps` 支持跨项目或项目内的创作灵感记录。
- **API 用量**：SQLite feature 下提供 `insert_api_usage`、`query_usage_by_model` 和 `query_usage_summary`。
- **CSV 包导入导出**：SQLite feature 下提供 `.fcworld` / CSV bundle 相关类型和流程。
- **Snapshot 版本管理**：默认 `snapshot` feature 使用 `git2` 保存 SQLite 数据快照。

## 技术栈

- Rust Edition 2024。
- 数据库：`sqlx`，默认 SQLite，可选 PostgreSQL。
- ID：`uuid` v7。
- 错误处理：`thiserror`，导出 `worldflow_core::Result`。
- 快照：`git2`，仅在 `snapshot` feature 下启用。
- CSV：`csv` crate。
- 系统信息：`sysinfo` 用于 SQLite 内存参数自适应。

## 目录结构

```text
core_world_data/
├── docs/              # 数据库、快照和接口变更设计文档
├── examples/          # 示例入口（当前为空或按需补充）
├── migrations/        # SQLite 迁移
├── migrations_pg/     # PostgreSQL 迁移
├── src/               # 库源码
├── tests/             # SQLite、快照、压力和 PostgreSQL 门控测试
├── Cargo.toml         # crate、features 和测试配置
├── LICENSE            # MIT 许可证
├── AGENTS.md          # AI 编码助手维护指南
└── Readme.md          # 当前文档
```

`src/` 模块职责：

- `lib.rs`：公共导出入口。
- `models/`：Project、Category、Entry、TagSchema、EntryRelation、EntryType、IdeaNote 等数据模型。
- `db/traits.rs`：统一数据库操作 trait。
- `db/mod.rs`：`SqliteDb` / `PgDb` 连接初始化、迁移和共享工具。
- `db/*`：SQLite 业务实现。
- `db/pg_*`：PostgreSQL 业务实现。
- `db/csv_bundle.rs`：CSV bundle 导入导出。
- `db/snapshot.rs`：SQLite 快照版本管理。

## 关键约定

- SQLite 初始化会自动开启 `foreign_keys`、`WAL`、`synchronous=NORMAL`，并运行 `migrations/`。
- SQLite 初始化会修复仅由 LF / CRLF 行尾差异导致的旧 SQLx migration checksum，真实迁移内容不一致仍会失败。
- PostgreSQL 初始化会运行 `migrations_pg/`。
- `.gitattributes` 已强制 `migrations/*.sql` 和 `migrations_pg/*.sql` 使用 LF 行尾，迁移文件不要随编辑器自动改成 CRLF。
- 修改模型或迁移时，需要同步考虑 SQLite 和 PostgreSQL 两套实现。
- `UpdateEntry.summary` 使用三态语义：不修改、清空、设置新值。
- `UpdateProject.description` 和 `cover_image` 也是三态更新。
- 删除项目依赖数据库外键级联清理关联数据。

## 许可证

MIT License，详见 `LICENSE`。

## 贡献方式

提交前请根据改动范围运行 `cargo test --lib`；涉及 PostgreSQL 编译兼容时额外运行 `cargo check --lib --no-default-features --features postgres`，涉及 PostgreSQL 行为或迁移时再运行 `cargo test --test stress_test_pg --no-default-features --features postgres`。涉及迁移、公共模型或 trait 的改动，需要同步更新 App 调用方和相关文档。
