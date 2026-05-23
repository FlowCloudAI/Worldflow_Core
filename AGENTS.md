# core_world_data — AGENTS.md

## 项目概览

`core_world_data`（Rust crate 名：`worldflow_core`）是 FlowCloudAI 的世界观数据核心库，向上层应用提供统一的项目、分类、词条、标签、关系、链接、想法笔记、API 用量和快照接口。默认面向 SQLite，本仓库同时保留 PostgreSQL feature 与迁移实现。

## 构建 / 运行 / 测试 / lint

```bash
cd core_world_data

# 默认 SQLite + snapshot feature
cargo check --lib
cargo test --lib

# 运行全部当前可用测试
cargo test

# PostgreSQL 编译检查
cargo check --lib --no-default-features --features postgres

# PostgreSQL 门控测试（需要可用数据库环境时再运行）
cargo test --test stress_test_pg --no-default-features --features postgres
```

当前没有独立 lint 脚本；Rust 代码修改后至少运行对应 `cargo check` / `cargo test`。

## 代码风格与命名约定

- Rust 使用 Edition 2024 和标准命名：类型 `PascalCase`，函数 / 变量 / 模块 `snake_case`，常量 `SCREAMING_SNAKE_CASE`。
- 注释、文档和示例文本使用中文。
- 公共 API 返回 `worldflow_core::Result<T>`，错误类型为 `WorldflowError`。
- 不在公共路径使用 `unwrap()` / `expect()`；测试代码可按现有风格使用。
- 数据访问能力通过 `src/db/traits.rs` 的 trait 暴露，新增能力优先先定义 trait，再分别实现 SQLite / PostgreSQL。
- 修改迁移时保持 SQL 可读，不把业务逻辑藏进临时脚本。

## 目录结构与模块职责

```text
core_world_data/
├── docs/              # 设计文档和接口变更记录
├── migrations/        # SQLite 迁移
├── migrations_pg/     # PostgreSQL 迁移
├── src/
│   ├── db/            # 数据库连接、trait 和各实体实现
│   ├── models/        # 数据模型与输入结构
│   ├── error.rs       # WorldflowError
│   └── lib.rs         # crate 导出入口
├── tests/             # 功能测试、快照测试、压力测试
├── Cargo.toml         # features 与测试配置
├── LICENSE
└── Readme.md
```

重点模块：

- `db/mod.rs`：`SqliteDb::new`、`SqliteDb::new_with_snapshot`、`PgDb::new`、迁移执行和 SQLite PRAGMA。
- `db/traits.rs`：上层依赖的稳定接口边界。
- `db/project.rs` / `pg_project.rs` 等成对文件：SQLite 与 PostgreSQL 实现需要保持语义一致。
- `db/csv_bundle.rs`：`.fcworld` / CSV 导入导出，改动风险较高。
- `db/snapshot.rs`：基于 `git2` 的 SQLite 快照，默认 feature 开启。
- `models/`：跨仓库公共数据结构，改字段时必须检查 App 和文档。

## 提交信息与 PR 规范

- 提交信息默认使用中文，格式建议为“动词 + 范围 + 目的”，例如 `修正词条搜索分页`。
- 一个提交只包含一个明确任务，不混入格式化、迁移和无关重构。
- PR 说明应写明影响的数据库后端、迁移编号、运行过的测试命令和兼容性风险。

## 安全 / 禁止事项

- 不提交 `.env`、本地数据库、临时压力测试输出或真实用户数据。
- 不直接删除或重写已发布迁移；需要新迁移修正历史行为。
- 不只改 SQLite 而忘记 PostgreSQL；确实不支持时在 README / AGENTS 标明 TODO。
- 不改 `Cargo.toml` 的 default features，除非同时确认 App、测试和发布流程。
- 不把 API Key、数据库密码或连接串写入示例。

## 项目特有坑点

- 默认 feature 是 `sqlite` + `snapshot`；PostgreSQL 必须用 `--no-default-features --features postgres` 显式检查。
- `src/db/sqlite.rs` 和 `src/db/postgres.rs` 当前只是模块占位，真实初始化逻辑在 `src/db/mod.rs`。
- `UpdateEntry.summary`、`UpdateProject.description`、`UpdateProject.cover_image` 使用三态更新，不能简化为普通 `Option<T>` 语义。
- `EntryFilter` 当前条件较少，新增过滤项需要同步 `count_entries`、`list_entries`、`search_entries`。
- SQLite 搜索依赖 FTS5；迁移改动后要确认触发器和 `entries_fts` 一致。
- `.gitattributes` 强制迁移 SQL 使用 LF；`SqliteDb::new` 只会修复旧库中由 LF / CRLF 差异导致的 migration checksum，不会掩盖真实迁移内容变更。
- `stress_test_pg` 有 `required-features = ["postgres"]`，普通 `cargo test` 不会覆盖 PostgreSQL 行为。
