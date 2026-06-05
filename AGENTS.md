# core_world_data — AGENTS.md

## 项目概览

`core_world_data` 是 FlowCloudAI 的世界观数据核心库，管理项目实体、关系、快照与迁移模型。  
它提供给桌面端与网站端的共享语义，版本兼容性主要取决于迁移链路与 `sqlx` 特性组合。

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

- Rust 使用 2024 Edition，类型 `PascalCase`，函数/变量 `snake_case`，常量 `SCREAMING_SNAKE_CASE`。  
- 数据模型与迁移改动应保持字段语义稳定并可追溯。  
- 错误返回保留上下文，便于跨服务排障。  

## 目录结构与模块职责

```text
core_world_data/
├── src/         # 数据模型、存储与查询实现
├── migrations/  # 版本迁移与升级脚本
├── tests/       # 回归测试与行为用例
└── .sqlx/       # 查询元数据缓存
```

## 安全 / 禁止事项

- 不提交真实数据库连接串、会话明文、用户隐私内容与签名信息。  
- 测试样例数据需脱敏，避免真实账号和可识别身份信息。  
- 变更迁移或索引前先评估性能与回滚影响。  

## 提交与 PR 规范

- 提交信息默认中文，单次 PR 聚焦迁移、模型或查询能力之一。  
- PR 说明应覆盖默认特性和 `--features sqlite` 两套验证结果。  
- 结构与测试变更需同步补充 `migrations` 与 `tests` 说明。  

## 项目特有坑点

- `sqlite` 与默认特性（含 `snapshot`）行为可差异明显，查询链路需双环境验证。  
- 快照、回放与上层世界观语义不一致时，可能放大版本兼容问题。  

文档同步时间：2026-06-05 12:44:21 +08:00
