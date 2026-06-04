# core_world_data — AGENTS.md

## 项目概览

`core_world_data` 是 FlowCloudAI 的世界观数据核心库，集中管理项目实体、关系、快照和迁移规则。  
上层应用与服务端通过该库共享模型与查询语义，版本兼容性主要取决于迁移和特性组合。

## 构建 / 运行 / 测试 / lint

```bash
cd core_world_data
cargo check --lib
cargo test --lib
cargo test
cargo check --lib --no-default-features --features sqlite
cargo test --no-default-features --features sqlite
```

## 代码风格与命名约定

- Rust 2024，类型 `PascalCase`，函数/变量 `snake_case`，常量 `SCREAMING_SNAKE_CASE`。  
- SQL 模型变更保持字段语义稳定，迁移文件与模型结构需可追溯。  
- 错误返回保留上下文信息，便于跨层排障。  

## 目录结构与模块职责

```text
core_world_data/
├── src/
│   ├── db/         # 存储与查询服务
│   └── models/     # 数据模型与实体定义
├── migrations/     # SQLite/版本迁移
├── tests/          # 回归测试
└── .sqlx/          # 查询缓存与元数据
```

## 安全 / 禁止事项

- 不提交真实数据库连接串、用户隐私内容与签名信息。  
- 测试样例数据需脱敏，不得包含外部可识别账号信息。  
- 迁移与索引改动需先评估回滚和查询性能影响。  

## 提交与 PR 规范

- 提交信息默认中文，单次变更聚焦模型、迁移或查询能力之一。  
- PR 需说明 `sqlite` 与默认特性两套验证结果。  
- 变更 schema 时同步补充 `tests` 与 `migrations` 说明。  

## 项目特有坑点

- SQLite 默认特性与 `--features sqlite` 下行为可能存在差异，查询兼容性需覆盖两条链路。  
- 快照、版本回放接口与上层 AI / 世界观口径不一致时会导致回放异常。  

## 文档同步依据（本次核对）

- 同步时间：2026-06-04 17:03:10 +08:00
- 依据文件：`core_world_data/Cargo.toml`、`core_world_data/src`、`core_world_data/migrations`、`core_world_data/tests`、`core_world_data/.sqlx`
