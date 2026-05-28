# core_world_data — AGENTS.md

## 项目概览

`core_world_data` 是世界观数据核心库，提供项目实体、关系图谱、快照和双数据库兼容的统一访问能力。  
上层应用通过该库复用数据模型，避免 SQLite 与 PostgreSQL 的差异直接外泄到业务层。

## 构建 / 运行 / 测试 / lint

```bash
cd core_world_data
cargo check --lib
cargo test --lib
cargo test
cargo check --lib --no-default-features --features postgres
cargo test --test stress_test_pg --no-default-features --features postgres
```

## 代码风格与命名约定

- Rust 2024，公开接口使用清晰 trait 边界与错误传播。  
- 新建查询/迁移时保持 `sqlite` 与 `postgres` 行为等价解释。  
- SQL 与 schema 变更需注明兼容范围与回退策略。

## 目录结构与职责

```text
core_world_data/
├── src/            # 模型、仓储、查询与服务接口
├── migrations/     # SQLite 迁移
├── migrations_pg/  # PostgreSQL 迁移
├── tests/          # 回归与压力测试
└── .sqlx/          # sqlx 缓存
```

## 安全 / 禁止事项

- 不提交数据库连接串、Token、生产环境凭据。  
- 测试数据可提交，敏感数据脱敏后再纳入仓库。  
- 变更索引前评估两个后端查询计划与性能回归。

## 提交与 PR 规范

- 同步提交 SQLite 与 PostgreSQL 的最小迁移与测试命令输出。  
- PR 说明包含迁移影响、兼容性风险、回滚方案。  
- 提交信息默认中文。

## 项目特有坑点

- SQLite 与 PostgreSQL 语义差异（事务、索引、文本匹配）常导致行为分叉。  
- `postgres` 特性命令依赖外部数据库环境，缺失时需在 PR 中显式说明跳过原因。

## 文档同步依据（本次核对）

- 同步时间：2026-05-28 18:02:58 +08:00  
- 依据文件：`core_world_data/Cargo.toml`、`core_world_data/migrations`、`core_world_data/migrations_pg`
