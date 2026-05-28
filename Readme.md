# 世界观数据核心（core_world_data）

`core_world_data` 是 FlowCloudAI 的世界观数据核心库，统一维护项目实体、关系图、快照与版本化存储。  
仓库同时提供 SQLite 与 PostgreSQL 的统一接口，供桌面端与服务端复用。

## 快速开始

### 安装与校验

```bash
cd core_world_data
cargo check --lib
cargo test --lib
```

### 最小示例

1. 运行 `cargo test --lib` 验证数据模型与抽象层。  
2. 运行 `cargo test` 验证端到端行为。  
3. 需要 PostgreSQL 时按特性运行对应命令。

## 主要功能 / 使用方式

- 数据建模、持久化与实体版本控制。  
- 快照与回放接口。  
- SQLite/ PostgreSQL 双后端兼容入口。  
- 关系图与查询工具链。  

## 技术栈

- Rust 2024、`sqlx`、迁移脚本、特性化编译。  
- `csv` 与 UUID、JSON 工具支持元数据导入导出。  

## 目录结构（仅顶层）

```text
core_world_data/
├── src/
├── migrations/
├── migrations_pg/
├── tests/
└── .sqlx/
```

## 许可证与贡献方式

许可证以子仓库声明为准。  
提交前补充 SQLite 与 PostgreSQL 的验证命令输出和兼容性说明。
