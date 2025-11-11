# Raster/Image 实体调研（阶段 2 补充）

本备忘用于汇总 ZCAD Rust 版在阶段 2 关于 Raster/Image（DXF `IMAGE`/`IMAGEDEF`/`IMAGEDEF_REACTOR`）实体的调研结果，并确认下一步实现路径。

## Pascal 版现状（待补充细节）

- 源码中重点图像渲染仍依赖 Pascal 侧的 ZGL 管线；`cad_source/zengine/core/utils/uzerasterizer.pas` 定义了打印/光栅化相关流程，但未直接暴露 DXF `IMAGE` 实体的加载逻辑。
- 需进一步确认 `cad_source/zengine/entities` 或导入器中是否存在专门的 IMAGE 解析（初步检索未发现），推测可能依赖外部库（如 LibreDWG）或仍在 TODO 列表。

## DXF 规范要点

- 参考 AutoCAD DXF Reference（[IMAGE Entity](https://help.autodesk.com/view/OARX/2024/ENU/?guid=GUID-3F14F28F-7E68-4E1C-B78B-3183EAD1CC81)）：  
  - `IMAGE` 实体通过组码 `340` 关联 `IMAGEDEF`，后者维护位图源文件、分辨率及显示选项。  
  - 关键几何字段：插入点（10/20/30）、U/V 方向向量（11/21/31、12/22/32）、图像大小（13/23、14/24）、裁剪边界（裁剪多边形或矩形）。  
  - 显示开关与属性：可见性、亮度/对比度/渐隐、显示裁剪边界等。
- DXF 还包含 `ACAD_IMAGE_DICT`/`ACAD_IMAGE_VARS` 字典对象，需要在文档装载阶段注册并建立 handle 关联，否则 `IMAGE` 实体会处于悬挂状态。

## Rust 版差距

| 项目 | 现状 | 目标 |
| --- | --- | --- |
| 数据结构 | ✅ `zcad-core` 已提供 `RasterImage`/`RasterImageDefinition`（含显示选项） | 扩展裁剪、多图像定义缓存等高级属性 |
| 解析流程 | ✅ `zcad-io` 支持 `IMAGE`/`IMAGEDEF` 基础字段与 handle 关联 | 针对裁剪边界、图像字典/变量做完整解析 |
| 资源加载 | 无加载/缓存逻辑 | 规划 `zcad-frontend` 或运行时资源包管理图像文件 |
| 渲染 | 无 | 评估交互前端如何在 `bevy`/`wgpu` 中呈现位图与裁剪 |

## 下一步建议

1. **确认 Pascal 行为**：定位 Pascal 版 `IMAGE` 导入与渲染流程，梳理必要字段及行为（裁剪、亮度等）。
2. ~~**建模草稿**：在 `zcad-core` 新增 `RasterImage`/`RasterImageDefinition` 数据类型，并定义最小的裁剪与显示属性。~~（已完成，详见 `image_basic` 黄金样例）
3. ~~**解析阶段拆分**：在 `zcad-io` 支持 `IMAGEDEF`/`IMAGE` 基本字段与 handle 关联，将未实现字段记录为 `UnsupportedFeature` 提示。~~（已完成，待扩展裁剪/字典数据）
4. **资源路径策略**：决定 DXF 外部文件的查找/拷贝策略（相对路径、`XREF` 目录、运行时资源包），参考 `rust/docs/raster_resource_strategy.md` 当前落实情况（定位器已就绪）。
5. **裁剪与渲染计划**：按照 `rust/docs/raster_clip_plan.md` 的路线图实现裁剪边界与字典解析，并在 Bevy 前端验证渲染。

> 备注：如需更深入的 DXF 示例，可复用 `errors/ltypeerror.dxf` 中的 `IMAGE` 案例或编写独立黄金样例，以便 `zcad-io` 解析测试。
