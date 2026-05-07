# 审计修复接口变动说明

本文记录本轮审计修复后对外部调用方可见的行为变化，便于 `worldflow_core` 直接使用方和桌面端 vendor 调用方同步迁移。

## 三态更新字段

`UpdateProject.description` 从 `Option<String>` 改为 `Option<Option<String>>`：

- `None`：不更新项目描述。
- `Some(None)`：清空项目描述，写入数据库 `NULL`。
- `Some(Some(value))`：写入新的描述字符串，允许真实空字符串 `""`。

`UpdateEntry.summary` 从 `Option<String>` 改为 `Option<Option<String>>`：

- `None`：不更新词条摘要。
- `Some(None)`：清空词条摘要，写入数据库 `NULL`。
- `Some(Some(value))`：写入新的摘要字符串，允许真实空字符串 `""`。

SQLite 与 PostgreSQL 的更新 SQL 已统一为三态语义。旧调用方如果只是构造 `Some(text)`，需要改成 `Some(Some(text))`；如果需要清空字段，传 `Some(None)`。

## Snapshot 改为手动触发

Snapshot 不再承诺写操作后自动创建快照，也不再保留内部 fire-and-forget 自动快照入口。调用方需要在合适的业务时机显式调用：

- `snapshot()`
- `snapshot_with_message(message)`

`SnapshotState::new` 现在会返回错误，不再把权限错误、损坏仓库、非法分支状态等问题静默降级为 `main`。只有明确缺少活动分支文件或尚未形成可用 Git HEAD 时，才会使用默认 `main`。

## Snapshot CSV 空值编码

新生成的 Snapshot CSV 对可选字符串字段使用 JSON 标量编码：

- `null` 表示数据库 `NULL`。
- `""` 表示真实空字符串。
- `"文本"` 表示普通字符串。

导入逻辑仍保留旧格式兼容：旧快照中的空单元格会按旧约定解释为 `NULL`。因此旧快照可继续恢复，但新快照可以区分 `NULL` 与真实空字符串。

## Entry Type 写入校验

`create_entry`、`update_entry` 和 `create_entries_bulk` 在写入 `Entry.r#type` 前会做一致性校验：

- `None` 允许，表示未指定类型。
- 非 UUID 字符串必须是内置词条类型 key。
- UUID 字符串必须对应当前项目下存在的自定义词条类型。
- 不存在的自定义类型、跨项目自定义类型、未知内置 key 都会返回 `InvalidInput`。

这会拒绝过去可能被静默写入的无效 `type` 字符串。

## SQLite 搜索摘要字段

SQLite FTS 新增 `summary` 字段，并通过 `0005_entries_fts_summary.sql` 迁移重建 FTS 表和触发器。长度大于等于 3 的 SQLite 搜索现在会与 PostgreSQL 一样命中 `title + summary + content`。

已有数据库升级时会回填当前 `entries.summary` 到 FTS 表；后续 insert/update/delete 由触发器同步。

## App 调用方迁移

桌面端 Tauri API 为兼容 TypeScript 的 `undefined/null/string` 区分，新增显式更新标记：

- `db_update_project`：`description_set` 表示是否更新描述。
- `db_update_entry`：`summary_set` 表示是否更新摘要。

前端封装规则：

- 字段为 `undefined`：不更新，对应 Rust `None`。
- 字段为 `null`：清空，对应 Rust `Some(None)`。
- 字段为字符串：写入新值，对应 Rust `Some(Some(value))`。

AI 工具侧传入 `Option<Option<String>>` 的摘要更新也已同步，避免把清空请求误转换为空字符串。
