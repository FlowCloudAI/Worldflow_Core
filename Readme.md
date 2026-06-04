# 世界观数据核心（core_world_data）

`core_world_data` 是 FlowCloudAI 的数据核心，提供世界观实体、关系图、版本快照与迁移能力，供桌面端和网站服务复用统一语义。  
它负责在数据库层保证数据一致性，并通过特性开关区分运行时行为。

## 项目简介

仓库覆盖模型定义、关系查询、快照回放与迁移体系，核心目标是维持历史兼容与可回滚性。  
改动通常会影响 `app_main` 与 `site_flowcloudai` 的世界观视图，需要同步跨仓库验证。

## 快速开始

### 安装与校验

```bash
cd core_world_data
cargo check --lib
cargo test --lib
cargo test
cargo check --lib --no-default-features --features sqlite
cargo test --no-default-features --features sqlite
```

### 最小示例

1. 运行 `cargo test --lib`，确认基础模型与查询行为。  
2. 运行 `cargo test`，确认集成场景回归。  
3. 对比 `sqlite` 特性与默认特性组合，记录查询差异。  

## 主要功能 / 使用方式

- 世界观项目、实体与关系模型定义。  
- 快照生成、版本回放和迁移管理。  
- SQL 查询策略与数据库特性兼容控制。  

## 技术栈

- Rust 2024、sqlx、git2、UUID、JSON  

## 目录结构（仅顶层）

```text
core_world_data/
├── src/
├── migrations/
├── tests/
└── .sqlx/
```

## 许可证与贡献方式

- 许可证：`core_world_data/LICENSE`。  
- 贡献前建议补充 `cargo test --lib`、`cargo test` 与迁移回滚步骤。  
- PR 说明应写明 SQLite 与非 SQLite 模式下的行为差异。  

文档同步时间：2026-06-04 17:03:10 +08:00
