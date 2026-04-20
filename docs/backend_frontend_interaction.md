# Worldflow 前后端交互数据流分析

## 核心实体关系

```
Project (项目/世界观)
  ├── Category (分类树) - 多对一关系
  │   └── Entry (词条) - 多对一关系
  │       └── EntryTag (词条标签值) - 关联 TagSchema
  │       └── FCImage (词条图片)
  ├── TagSchema (标签定义) - 一对多关系
  └── AppSetting (配置)
```

---

## 前端需要的数据类型详解

### 📋 1. 项目列表页 (Dashboard)

**前端需要的数据：**
```typescript
// 获取用户的所有项目
GET /api/projects
Response: Project[]
[
  {
    id: "proj_123",
    name: "魔法世界",
    description: "一个奇幻魔法设定",
    created_at: "2024-01-01T10:00:00Z",
    updated_at: "2024-03-21T15:00:00Z"
  }
]
```

**前端交互：**
- 显示项目卡片列表
- 创建新项目
- 删除项目
- 进入项目详情

---

### 🌳 2. 项目详情 - 分类管理

**前端需要的数据：**

```typescript
// 获取项目的分类树
GET /api/projects/{projectId}/categories
Response: CategoryTree[]
[
  {
    id: "cat_1",
    project_id: "proj_123",
    parent_id: null,
    name: "人物",
    sort_order: 1,
    created_at: "...",
    updated_at: "...",
    children: [
      {
        id: "cat_1_1",
        project_id: "proj_123",
        parent_id: "cat_1",
        name: "主角",
        sort_order: 10,
        children: []
      }
    ]
  },
  {
    id: "cat_2",
    project_id: "proj_123",
    parent_id: null,
    name: "物品",
    sort_order: 2,
    children: []
  }
]
```

**前端交互：**
- 展示树形分类
- 创建/删除分类
- 拖拽排序
- 移动分类（parent_id更新）
- 检验循环引用（后端处理）

---

### 📝 3. 词条列表页

**前端需要的数据（关键是用轻量级的EntryBrief）：**

```typescript
// 获取某分类下的词条列表
GET /api/projects/{projectId}/entries?category_id={catId}&limit=20&offset=0
Response: EntryListResponse
{
  items: [
    {
      id: "entry_1",
      project_id: "proj_123",
      category_id: "cat_1_1",
      title: "亚瑟王",
      type: "character",
      cover: "/images/arthur.jpg",      // 第一张is_cover=true的图片
      updated_at: "2024-03-20T10:00:00Z"
    }
  ],
  total: 45,
  limit: 20,
  offset: 0
}
```

**前端交互：**
- 列表分页展示
- 搜索词条
  ```typescript
  GET /api/projects/{projectId}/entries/search?q=亚瑟&limit=10
  Response: SearchEntryResult
  ```
- 创建新词条
- 删除词条
- 进入词条详情

---

### 📖 4. 词条详情页

**前端需要的完整数据：**

```typescript
// 获取单个词条的完整信息
GET /api/projects/{projectId}/entries/{entryId}
Response: Entry
{
  id: "entry_1",
  project_id: "proj_123",
  category_id: "cat_1_1",
  title: "亚瑟王",
  content: "英雄的故事...",
  type: "character",
  images: [
    {
      path: "/storage/img1.jpg",
      is_cover: true,
      caption: "年轻时的亚瑟"
    },
    {
      path: "/storage/img2.jpg",
      is_cover: false,
      caption: "成年后的肖像"
    }
  ],
  tags: [
    {
      schema_id: "tag_schema_1",
      value: 80                         // 对应TagSchema type="number"
    },
    {
      schema_id: "tag_schema_2",
      value: "human"                    // 对应TagSchema type="string"
    },
    {
      schema_id: "tag_schema_3",
      value: true                       // 对应TagSchema type="boolean"
    }
  ],
  created_at: "2024-01-15T10:00:00Z",
  updated_at: "2024-03-20T10:00:00Z"
}
```

**前端交互：**
- 编辑标题、内容
- 修改分类
- 上传/删除图片
- 编辑标签值
- 更新词条

---

### 🏷️ 5. 标签管理页

**前端需要的数据：**

```typescript
// 获取项目的所有标签模式
GET /api/projects/{projectId}/tag-schemas
Response: TagSchema[]
[
  {
    id: "tag_schema_1",
    project_id: "proj_123",
    name: "力量值",
    description: "角色的力量数值",
    type: "number",
    target: ["character"],              // 只对人物适用
    default_val: "50",
    range_min: 0,
    range_max: 100,
    sort_order: 1,
    created_at: "...",
    updated_at: "..."
  },
  {
    id: "tag_schema_2",
    project_id: "proj_123",
    name: "种族",
    description: "角色的种族",
    type: "string",
    target: ["character", "creature"],  // 对人物和生物都适用
    default_val: "human",
    range_min: null,
    range_max: null,
    sort_order: 2,
    created_at: "...",
    updated_at: "..."
  },
  {
    id: "tag_schema_3",
    project_id: "proj_123",
    name: "已死亡",
    description: "角色是否已死亡",
    type: "boolean",
    target: ["character"],
    default_val: "false",
    range_min: null,
    range_max: null,
    sort_order: 3,
    created_at: "...",
    updated_at: "..."
  }
]
```

**前端交互：**
- 创建标签模式
- 编辑标签定义
- 删除标签模式
- 排序标签

---

## 前端表单数据示例

### 创建词条表单
```typescript
const createEntryForm = {
  project_id: "proj_123",
  category_id: "cat_1_1",
  title: "新人物",
  content: "详细描述",
  type: "character",
  
  // 上传的图片列表
  images: [
    { path: "/storage/new1.jpg", is_cover: true, caption: "主图" }
  ],
  
  // 标签值（需要从TagSchema列表中选择）
  tags: [
    { schema_id: "tag_schema_1", value: 75 },
    { schema_id: "tag_schema_2", value: "elf" },
    { schema_id: "tag_schema_3", value: false }
  ]
}
```

### 更新词条表单
```typescript
const updateEntryForm = {
  // 只更新需要改动的字段（可选）
  title: "更新后的名称",
  content: "新的详细描述",
  
  // 更新分类为null表示移到根节点
  category_id: null,
  
  // 标签值完整替换
  tags: [
    { schema_id: "tag_schema_1", value: 90 }
  ]
}
```

---

## 关键设计点

### ✅ 为什么需要分离 Entry 和 EntryBrief？

| 场景 | 使用类型 | 原因 |
|------|--------|------|
| 列表展示 | `EntryBrief[]` | content 和 tags 数据量大，不需要全部加载，减少网络传输 |
| 详情页 | `Entry` | 需要完整内容和所有标签进行编辑 |
| 搜索结果 | `EntryBrief[]` | 同列表展示，只显示摘要 |

### ✅ 标签设计的灵活性

```typescript
// 同一个TagSchema可以被多个Entry引用
// 字段值的type由TagSchema.type定义，保证一致性

// 例如：同一个"力量值"标签在多个角色上使用
Entry 1: { schema_id: "tag_schema_1", value: 80 }
Entry 2: { schema_id: "tag_schema_1", value: 65 }
Entry 3: { schema_id: "tag_schema_1", value: 95 }
```

### ✅ 分类树的前端处理

```typescript
// 后端返回扁平的Category列表
// 前端需要自己构建树形结构，或直接返回CategoryTree

// 前端排序方式：parent_id NULLS FIRST, sort_order, name
// 这样能保证根节点在前，同级按sort_order排列
```

### ✅ 图片处理

```typescript
// FCImage 的 path 可能是：
// 1. 本地存储路径：/storage/uuid.jpg
// 2. URL: https://cdn.example.com/image.jpg
// 3. 相对路径：./images/local.jpg

// 前端需要判断是否能直接访问，或需要通过API获取
```

---

## 前端开发检查清单

- [ ] 项目列表：使用 `Project[]`
- [ ] 分类树：使用 `CategoryTree[]`
- [ ] 词条列表：使用 `EntryBrief[]` + 分页
- [ ] 词条详情：使用 `Entry`
- [ ] 标签管理：使用 `TagSchema[]`
- [ ] 创建词条：使用 `CreateEntryRequest`
- [ ] 更新词条：使用 `UpdateEntryRequest`
- [ ] 分类选择器：转换 `Category[]` 为 `CategoryOption[]`
- [ ] 标签编辑：转换 `EntryTag[]` + `TagSchema[]` 为 `TagValueInput[]`
- [ ] 搜索功能：使用 `SearchEntryResult`
- [ ] 错误处理：使用 `ApiError` 和 `ApiResponse<T>`