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
- 高频写入会积累大量提交；批量导入场景建议用 `SqliteDb::new()` 导入完再手动调一次 `snapshot()`

---

## 公共 API

```rust
// 手动打一个快照（消息前缀 "manual <unix_secs>"）
db.snapshot().await?;

// 列出所有历史快照，最新的在 index 0
let versions: Vec<SnapshotInfo> = db.list_snapshots().await?;
// SnapshotInfo { id: String, message: String, timestamp: i64 }

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

- 实现：[src/db/snapshot.rs](src/db/snapshot.rs)
- 公开导出：[src/lib.rs](src/lib.rs) — `#[cfg(feature = "sqlite")]`
- 测试：[tests/snapshot.rs](tests/snapshot.rs)（14 个用例）

```bash
cargo test --features sqlite --test snapshot
```
