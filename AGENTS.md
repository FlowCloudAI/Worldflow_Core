# core_world_data — AGENTS.md

## 项目概览

`core_world_data` 是世界观数据核心库，提供项目实体、关系图谱、快照和 SQLite 数据访问能力。  
上层应用通过该库复用数据模型、迁移与快照能力。

## 构建 / 运行 / 测试 / lint

```bash
cd core_world_data
cargo check --lib
cargo test --lib
cargo test
cargo check --lib --no-default-features --features sqlite
```

## 代码风格与命名约定

- Rust 2024，公开接口使用清晰 trait 边界与错误传播。  
- 新建查询/迁移时保持 SQLite schema、模型和测试同步。  
- SQL 与 schema 变更需注明兼容范围与回退策略。

## 目录结构与职责

```text
core_world_data/
├── src/            # 模型、仓储、查询与服务接口
├── migrations/     # SQLite 迁移
├── tests/          # 回归与压力测试
└── .sqlx/          # sqlx 缓存
```

## 安全 / 禁止事项

- 不提交数据库连接串、Token、生产环境凭据。  
- 测试数据可提交，敏感数据脱敏后再纳入仓库。  
- 变更索引前评估 SQLite 查询计划与性能回归。

## 提交与 PR 规范

- 同步提交 SQLite 迁移、模型变更与测试命令输出。  
- PR 说明包含迁移影响、兼容性风险、回滚方案。  
- 提交信息默认中文。

## 项目特有坑点

- SQLite FTS5、事务、索引和触发器语义对查询行为影响大，改动后需补充回归验证。  
- `snapshot` 特性依赖 `git2`，禁用默认特性时需显式选择所需能力。

## 文档同步依据（本次核对）

- 同步时间：2026-05-28 18:02:58 +08:00  
- 依据文件：`core_world_data/Cargo.toml`、`core_world_data/migrations`
