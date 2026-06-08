# 世界观数据核心（core_world_data）

## 项目简介

`core_world_data` 是世界观实体、关系和快照的数据核心，提供桌面端与网站端共用的模型、查询与迁移能力。  
该库变更会直接影响上层语义链路，需在默认与 `sqlite` 双特性下验证一致性。

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
3. 分别对比默认特性与 `sqlite` 链路，确认快照与查询差异。  

## 主要功能 / 使用方式

- 世界观项目、实体与关系模型定义。  
- 快照生成、版本回放与迁移管理。  
- 默认与 `sqlite` 特性并行验证。  

## 技术栈

- Rust 2024、sqlx、git2、serde、uuid

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
- PR 前建议补充 `cargo test --lib`、`cargo test` 与迁移回滚验证结果。  
- 变更说明需写清默认与 `sqlite` 模式下行为差异。  

文档同步时间：2026-06-08 13:20:10 +08:00
