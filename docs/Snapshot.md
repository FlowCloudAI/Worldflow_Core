# Snapshot — SQLite 版本管理

仅 `sqlite` feature 可用。不依赖系统 git，使用 `git2` crate 在指定目录管理 CSV 文件。

---

## 初始化

```rust
use worldflow_core::{SqliteDb, SnapshotConfig};

let db = SqliteDb::new_with_snapshot(
"sqlite:./data.db?mode=rwc",
SnapshotConfig {
dir: PathBuf::from("./snapshots"),   // CSV + git repo 存放目录
author_name: "App".to_string(),
author_email: "app@example.com".to_string(),
},
).await?;
```

`SqliteDb::new()` 不带快照，所有 snapshot 方法对它返回 `Err(SnapshotNotConfigured)`。

---

## 自动快照

每次写操作（create / update / delete）自动触发一次 fire-and-forget 异步快照，消息前缀为 `"auto <unix_secs>"`。

- 内部用 `Mutex` 串行化，不会并发提交
- 失败只打印到 stderr，不传播给调用方
- 自动快照始终提交到当前活动分支
- 高频写入会积累大量提交；批量导入场景建议用 `SqliteDb::new()` 导入完再手动调一次 `snapshot()`

---

## 活动分支

Snapshot 支持应用层“活动分支”概念：

- `snapshot()` / `snapshot_with_message()` / 自动快照 / `rollback_to()` 前保护快照，都会提交到活动分支
- `list_snapshots()` 默认只查看活动分支历史
- `switch_branch()` 会把数据库恢复到目标分支 tip，然后把活动分支切过去

活动分支会持久化到 `SnapshotConfig::dir/.worldflow-active-branch`，因此重建 `SqliteDb` 后会恢复上次活动分支。

---

## 公共 API

```rust
// 手动打一个快照（消息前缀 "manual <unix_secs>"）
db.snapshot().await?;

// 手动打一个带自定义提交信息的快照
db.snapshot_with_message("整理角色设定").await?;

// 当前活动分支
let branch = db.active_branch().await?;

// 列出所有分支
let branches: Vec<SnapshotBranchInfo> = db.list_branches().await?;
// SnapshotBranchInfo { name, head, is_current, is_active }

// 创建分支。未传 from_ref 时，默认从当前活动分支分出
db.create_branch("feature/role-rewrite", None).await?;

// 切换活动分支，并把数据库恢复到目标分支 tip
db.switch_branch("feature/role-rewrite").await?;

// 列出所有历史快照，最新的在 index 0
let versions: Vec<SnapshotInfo> = db.list_snapshots().await?;
// SnapshotInfo { id: String, message: String, timestamp: i64 }

// 查看指定分支历史
let feature_versions = db.list_snapshots_in_branch("feature/role-rewrite").await?;

// 直接把当前数据库状态提交到指定分支
db.snapshot_to_branch("feature/role-rewrite", "补充角色背景").await?;

// 回退：先自动保存 pre-rollback 快照，再全量替换数据库
db.rollback_to( & versions[2].id).await?;

// 追加恢复：把历史快照里有、当前 DB 里没有的记录补回来（非破坏性）
let result: AppendResult = db.append_from( & versions[1].id).await?;
// AppendResult { projects, categories, entries, tag_schemas,
//                relations, links, entry_types, idea_notes }

// 从磁盘上的 CSV 目录恢复（dir 需含 8 个 CSV 文件）
db.restore_from_csvs( & dir, RestoreMode::Replace).await?;  // 先清空再导入
db.restore_from_csvs( & dir, RestoreMode::Merge).await?;    // 只补缺失记录
```

`snapshot_id` 接受完整 SHA 或短 SHA（git2 `revparse_single` 语义）。

---

## 分支约束

- 分支是“有基线提交的分支”，不是空分支
- 如果当前没有任何基线提交，`create_branch()` 会返回 `InvalidInput("当前没有基线提交，不能创建分支")`
- 这意味着上层 APP 应在“首次快照”之前禁用分支创建入口
- `switch_branch()` 只允许切到已存在的本地分支
- `list_branches()` 同时返回：
    - `is_current`: Git 仓库当前 `HEAD` 分支
    - `is_active`: Worldflow 当前活动分支

`is_current` 与 `is_active` 可能不同，这是设计行为：活动分支由应用层管理，不依赖 checkout。

---

## 导出的 CSV 文件

每次快照写出 8 个文件到 `SnapshotConfig::dir`：

```
projects.csv  categories.csv  tag_schemas.csv  entry_types.csv
entries.csv   entry_relations.csv  entry_links.csv  idea_notes.csv
```

所有字段均为字符串；UUID 存连字符格式；可选字段为空字符串表示 NULL。

---

## 关键行为约束

| 场景                     | 行为                                                    |
|------------------------|-------------------------------------------------------|
| `rollback_to` 期间       | 持有锁全程，阻断自动快照插入中间状态                                    |
| `RestoreMode::Replace` | `PRAGMA foreign_keys = OFF` + 手动事务；依赖顺序删除 8 张表        |
| `RestoreMode::Merge`   | `INSERT OR IGNORE`，不删除现有记录                            |
| 分类插入顺序                 | 拓扑排序，父分类先于子分类，防止 FK 违反                                |
| FTS5 一致性               | SQLite 触发器在 FK OFF 下仍然触发，Replace 后 `entries_fts` 自动同步 |

---

## 相关文件

- 实现：[src/db/snapshot.rs](../src/db/snapshot.rs)
- 公开导出：[src/lib.rs](../src/lib.rs) — `#[cfg(feature = "sqlite")]`
- 库内分支测试：`src/db/snapshot.rs` 内 `db::snapshot::tests`

```bash
cargo test --lib snapshot::tests
```
