# core_world_data — AGENTS.md

## 项目概览

`core_world_data` 是 FlowCloudAI 的世界观数据核心库，负责项目实体、关系、快照和迁移模型。  
它是桌面端与站点端共享世界观语义的基础，版本兼容主要依赖迁移链路与 `sqlx` 特性组合。

## 构建 / 运行 / 测试 / lint

```bash
cd core_world_data
cargo check --lib
cargo test --lib
cargo test
cargo check --lib --no-default-features --features sqlite
cargo test --no-default-features --features sqlite
```

该仓库未配置独立 lint 脚本，需以双特性测试链路作为质量门槛。

## 代码风格与命名约定

- Rust 使用 2024 Edition，类型 `PascalCase`，函数/变量 `snake_case`，常量 `SCREAMING_SNAKE_CASE`。  
- 数据模型与迁移改动应保持字段语义稳定并可追溯。  
- 错误返回保留上下文信息，便于跨服务排障。  

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
- 测试样例需脱敏，避免真实账号或可识别身份信息。  
- 变更迁移或索引前先评估性能与回滚影响。  

## 提交与 PR 规范

- 提交信息默认中文，单次 PR 聚焦迁移、模型或查询能力之一。  
- PR 说明应覆盖默认特性和 `--features sqlite` 两套验证结果。  
- 结构与测试变更需同步补充 `migrations` 与 `tests` 说明。  

## 项目特有坑点

- `sqlite` 与默认特性（含 `snapshot`）行为可显著不同，需双环境验证。  
- 快照与回放语义若与上层世界观模型不一致，会放大版本兼容风险。  

文档同步时间：2026-06-08 13:20:10 +08:00
