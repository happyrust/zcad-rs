# 阶段 2 实体覆盖矩阵（初稿）

> 说明：表格用于追踪 ZCAD Rust 版在阶段 2 的几何/电气实体支持情况，以及与 Pascal 版的对齐与测试状态。Pascal 版现状基于公开模块与已有 DXF 样例，后续需结合原有文档进一步核对。

| 实体/结构 | Pascal 版支持情况 | Rust `zcad-core` 现状 | 黄金样例/测试 | 后续动作 |
| --- | --- | --- | --- | --- |
| Line | 常规绘图主力实体 | ✅ `Document::add_line` / `Entity::Line` | `tests/data/basic_entities.dxf` | 已覆盖 |
| Circle | 常规绘图主力实体 | ✅ `Document::add_circle` / `Entity::Circle` | `basic_entities.dxf` | 已覆盖 |
| Arc | 常规绘图主力实体 | ✅ `Document::add_arc` / `Entity::Arc` | `basic_entities.dxf` | 已覆盖 |
| Ellipse | DXF 扩展实体 | ✅ `Document::add_ellipse` / `Entity::Ellipse` | `ellipse_basic.dxf` | 已覆盖 |
| Polyline (含 bulge) | 常规绘图主力实体 | ✅ `Document::add_polyline` / `Entity::Polyline` | `bulge_polyline.dxf` | 已覆盖 |
| Text | 注释实体 | ✅ `Document::add_text` / `Entity::Text` | `basic_entities.dxf` | 已覆盖 |
| MText | 注释实体 | ✅ `Document::add_mtext` / `Entity::MText` | `mtext_basic.dxf` | 已覆盖 |
| Block Definition / Insert | 常规组件、符号库基础 | ✅ `BlockDefinition` / `Entity::BlockReference` | `block_insert.dxf`、`block_multiline.dxf` | 已覆盖 |
| Attribute / Attribute Definition | 块属性 | ✅ 结构已建模、DXF 解析支持 | `block_insert.dxf`、`block_multiline.dxf` | 关注多语言/编码 |
| Hatch / Solid | 常见填充实体 | ✅ `Entity::Hatch`（多环路/渐变/椭圆/样条/引用边界） | `hatch_simple.dxf`、`hatch_ellipse.dxf`、`hatch_spline.dxf` | 下一步扩展渐变色表与外部引用联动；结合前端验证交互 |
| Dimension（线性/角度等） | 设计标注关键 | ✅ `Entity::Dimension`（线性/对齐/角度/直径/半径/三点角度） | `dimension_linear.dxf`、`dimension_angular.dxf`、`dimension_diameter.dxf`、`dimension_radius.dxf`、`dimension_angular3pt.dxf` | 下一步聚焦坐标尺寸、样式参数与文本格式 |
| Leader / MLeader | 复杂注释需求 | ✅ 扩展缩放/狗腿/落脚间隙并解析多引线块内容 | `leader_entities.dxf`、`mleader_block.dxf`、`mleader_block_attrs.dxf`、`mleader_block_connections.dxf` | 持续监控交互体验，后续与 Bevy 渲染结果对照 |
| Spline | 高阶曲线 | ✅ `Entity::Spline`（控制点/拟合点/节点/切向量） | `spline_basic.dxf` | 后续补充更精细的曲线采样与重量级样例 |
| 3DFace / Mesh | 3D 扩展 | 🛠️ 3DFace 已建模并连通 DXF 解析/CLI/Bevy 预览，支持 POLYFACE 与 POLYGON MESH（含 wrap 标志）拆解为 3DFACE | `face3d_basic.dxf`、`polyface_basic.dxf`、`mesh_grid_basic.dxf`、`mesh_wrap_basic.dxf` | Mesh 渲染策略与更复杂 MESH 数据仍待确认 |
| Image / Raster | 外部参照 | ✅ 解析裁剪、字典、缺失文件回退与占位纹理 | `image_basic.dxf`、`image_clip_polygon.dxf`、`image_missing_file.dxf` | 后续关注性能与大图缓存策略 |
| 电气专用模块（符号、连线） | Pascal 版扩展 | ⏳ 数据结构需勘测 | 暂缺 | 与业务团队确认优先级与 DXF 表达方式 |
| 对象捕捉辅助几何 | 内部辅助结构 | ⏳ `zcad-core` 尚未抽象 | N/A | 确定是否在核心库或引擎层实现 |

## 行动项

1. **确认 Pascal 版实体清单**：通过原始代码（`cad_source/zcad/zengine/entities/*` 等）或维护者文档对表格做二次核对，补充遗漏实体。
2. **为缺失实体准备 DXF 样例**：每项至少准备一份简明 DXF 文件与预期黄金快照，避免后续解析回归缺口。
3. **讨论阶段范围**：对于高成本实体（如 Spline、3D Mesh、Raster），需与产品/业务确认是否纳入阶段 2，或移交至后续阶段。
4. **强化渲染验证**：完成 CLI 诊断输出后，需在 Bevy 前端进一步对照 Hatch 渐变/多环路与 Dimension 渲染并记录差异。
5. **Bevy 多引线渲染验证**：在新样例基础上对照 Pascal 版表现，补全交互测试。
6. **Raster 图像性能调优**：评估大尺寸/缺失资源场景下的缓存与日志策略，规划后续优化。

## 阶段 2 迁移任务线框图

```
┌───────────────────────────┐
│        数据输入层         │
│ ┌───────────────────────┐ │
│ │ DXF Loader / Parser  │◄┼─ 黄金样例维护 (tests/data + golden)
│ └───────────────────────┘ │
│           │               │
└───────────┼───────────────┘
            ▼
┌───────────────────────────┐
│        zcad-core          │
│ ┌───────┬────────┬──────┐ │
│ │Entity │Document│Geom  │ │
│ │(Line… │(Scene) │math) │ │
│ └───────┴────────┴──────┘ │
│   ▲           ▲           │
│   │Selection  │Viewport   │→ 待补辅助结构/对象捕捉
└───┼───────────┼──────────┘
    │           │
    ▼           ▼
┌───────────────────────────┐
│        zcad-engine        │
│ - ID/图层/属性生命周期     │
│ - 选择集/命令管线          │
│ - Hatch/Dimension 逻辑     │
└───────────┬───────────────┘
            │
            ▼
┌───────────────────────────┐
│       前端/诊断层          │
│ ┌───────────────┬────────┐ │
│ │ CLI Snapshot  │ Bevy   │ │
│ │  (报告/比较)  │ Viewer │ │
│ └───────────────┴────────┘ │
│   ▲                 ▲      │
│   │ Hatch/Dim 渐变  │ Mesh │
│   │ 渲染验证        │ 可视 │
└───┼─────────────────┼──────┘
    │                 │
    ▼                 ▼
文档同步 (`docs/stage2_entity_matrix.md`, `raster_*`)
```

> 线框图展示了“解析 → 核心建模 → 引擎逻辑 → 前端验证”的端到端路径，可用于跟踪阶段 2 每个子任务的输入、输出和依赖的文档/测试资源。
