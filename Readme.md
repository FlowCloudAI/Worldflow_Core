# 世界观数据核心（core_world_data）

`core_world_data` 提供世界观实体、关系和快照的统一数据层，用于桌面端与站点服务共享查询语义与迁移口径。

## 项目简介

仓库管理模型定义、关系查询、快照生成和迁移体系，目标是确保历史兼容与可回滚。  
核心变更通常会影响多个上层服务的世界观视图，需要按双特性链路验证。

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

1. 运行 `cargo test --lib`，确认模型与基础查询。  
2. 运行 `cargo test`，确认集成行为。  
3. 对比 `sqlite` 与默认特性结果，记录差异。  

## 主要功能 / 使用方式

- 世界观项目、实体和关系模型定义。  
- 快照生成、版本回放与迁移管理。  
- 支持默认与 `sqlite` 特性两套验证。  

## 技术栈

- Rust 2024、sqlx、git2、UUID、Serde/Serde JSON  

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
- PR 前请补充 `cargo test --lib`、`cargo test` 与迁移回滚验证。  
- 变更说明应写清默认与 `sqlite` 模式下的行为差异。  

文档同步时间：2026-06-05 12:44:21 +08:00
