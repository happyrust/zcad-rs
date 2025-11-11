# Raster 图像裁剪与字典解析计划

本文档概述 DXF `IMAGE` 裁剪边界与相关字典（`IMAGEDEF_REACTOR`、`ACAD_IMAGE_DICT` 等）的解析计划，补齐阶段 2 之后的功能清单。

## DXF 要点回顾

- `IMAGE` 实体可通过组码 `14/24/341` 等定义矩形或多边形裁剪边界；当 `use_clipping` 标志启用时，需要裁剪渲染输出。
- `IMAGEDEF_REACTOR` 负责维护图像实体与定义之间的关系；`ACAD_IMAGE_DICT` 与 `ACAD_IMAGE_VARS` 在 `OBJECTS` 段中描述全局设置（亮度、对比度默认值、阴影处理等）。
- 自 DXF R2000 起，裁剪多边形可包含任意数量的顶点，需关注顺时针/逆时针与闭合标志。

## 现状

- `zcad-core` 已引入裁剪多边形与图像字典相关的数据结构（`RasterImageClip`、`ImageDictionary`、`RasterImageVariables`），`RasterImage` 支持可选裁剪信息与 Reactor 句柄。
- `zcad-io` 已解析 `IMAGE` 裁剪组码、`IMAGEDEF_REACTOR` 以及 `ACAD_IMAGE_DICT` / `ACAD_IMAGE_VARS`，并在黄金测试中输出对应快照。
- 前端渲染尚未对图像裁剪做处理，也没有缓存/复用字典级属性。

## 计划任务

1. **数据结构设计（阶段 2 补充）** ✅ 已在 `zcad-core` 引入 `RasterImageClip` / `ImageDictionary` / `RasterImageVariables`，并扩展 `RasterImage` 支持裁剪信息与 Reactor 句柄。

2. **解析层实现（阶段 3）** ✅ `zcad-io` 已覆盖 `IMAGE` 裁剪组码、`IMAGEDEF_REACTOR`、`ACAD_IMAGE_DICT` 与 `ACAD_IMAGE_VARS` 的解析，并同步黄金测试与断言。

3. **前端集成（阶段 3 后期）**  
   - ✅ Bevy 前端已加载 Raster 图像纹理并基于裁剪顶点生成网格，同时绘制可选边框。  
   - 后续评估 GPU shader 方案与资源缓存策略，完善占位纹理与性能监控。  
   - 依据 `IMAGEDEF_REACTOR` 实现资源引用计数或共享缓存策略。

4. **测试与验证（阶段 3 后期）**  
   - 准备 DXF 样例，覆盖矩形裁剪、多边形裁剪以及缺失/损坏字典情况。  
   - 在 `zcad-io` snapshot 测试中比对裁剪顶点及字典信息；渲染端进行截图或像素对比测试。

## 近期调整（2025-11-07）

- 完成 `RasterImageClip` 数据结构、图像字典/变量解析及 `IMAGEDEF_REACTOR` 维护，黄金样例 `image_basic`、`image_clip_polygon` 已覆盖 Reactor 句柄与 ACAD 字典内容，前端裁剪渲染已落地在 Bevy 预览中。
- 已新增 `image_clip_polygon.dxf` 作为多边形裁剪黄金样例基础。
- 解析实现计划细化如下：
  - 在 `zcad-io` 的 `parse_image` 中引入裁剪状态机，识别矩形（组码 `14/24` 成对出现两次）与多边形（`91` 顶点数 + 多组 `14/24`）。  
  - 当 `76`（裁剪启用标志）为 0 时忽略裁剪数据。  
  - 解析结果存储到新的 `RasterImageClip`（矩形使用 `min/max`，多边形保存顶点顺序）。
  - `HatchEdge` 的 bulge 处理逻辑可复用以支持潜在弧形边界（若后续 DXF 版本引入）。  
- 文档与 changelog 将在实现完成后再更新记录。

## 依赖与风险

- 需确认 AutoCAD 不同版本对裁剪多边形的限制以及组码兼容性。  
- 渲染裁剪涉及 shader 或 CPU 合成，需评估性能与实现复杂度。  
- 行为需与 Pascal 版保持一致，避免造成历史项目的显示偏差。

## 下一步行动

1. **资源引用与缓存链路**  
   - 在 `zcad-frontend`/`zcad-app` 里沿用 `ImageDictionary` 与 `RasterImageVariables`，并初步把 `IMAGEDEF_REACTOR` 引导到统一的引用计数缓存层，避免多实例重复加载同一路径。
   - ✅ Bevy 前端已基于 `IMAGEDEF_REACTOR`、字典条目与全局 `RasterImageVariables` 构造统一缓存键，并在日志中提示缺失字典时回退路径，方便 CLI/Bevy 对照。
   - 规划运行时占位纹理与日志规则，当字典缺失值时输出诊断信息，便于 CLI 与 Bevy 前端对照结果。

2. **裁剪网格与 shader 方案验证**  
    - 对比 Bevy 渲染和 Pascal 版在多边形裁剪时的网格拓扑，确保顺序性与奇偶判断一致；必要时增加顶点归一化流程避免 winding 差异。
    - ✅ Bevy 端目前在 `raster_local_polygon` 中同步 Pascal 的裁剪边界（矩形/多边形）并依据 `clip_mode` 归一化 winding，同时在 `spawn_raster_image` 输出裁剪与纹理诊断日志，便于追踪与 Pascal 在渲染顺序、资源热替换与缺失纹理行为上的一致性。
   - 计划参考 Pascal 渲染路径（如 `cad_source/zengine/zgl/common/uzglviewareageneral.pas` 中的视口/裁剪矩阵处理以及 `cad_source/zengine/core/drawings/uzedrawingsimple.pas` 里的实体可见集构建）以及 `cad_source/components/fpdwg/libredwg/dwg.pp`/`cad_source/components/fpdwg/libredwg/dwg.h` 中 `IMAGE` 实体的 `clip_verts`/`clip_mode` 定义（注：`clip_mode` 在 `dwg.h:4309`/`4491` 注释为外部 vs. 内部）来确认 Bevy 的顶点采样与 Pascal 保持一致。
   - 评估 `wgpu` shader/compute 或 CPU 拷贝实现的性能，决定是否在 pipeline 中引入裁剪边界缓存。

3. **测试与黄金样例扩展**  
   - 追加带有 `ACAD_IMAGE_DICT` / `IMAGEDEF_REACTOR` 的黄金 DXF，如带有不同亮度/对比度/阴影设置的图片实体，并将其纳入 `zcad-io` snapshot 。
   - 记录 Bevy 渲染截图与 CLI 诊断输出，确保裁剪数据在热加载/资源替换等场景下仍然一致。
   - 考虑新增 `image_clip_dict_basic.dxf`（含字典引用与 Reactor）与 `image_clip_dict_runtime.dxf`（含不同 RasterImageVariables 参数）的样例，并在 `zcad-io/tests/data/golden` 中保留相应 JSON，与 CLI/Bevy 输出做像素/诊断对比。
   - ✅ `image_clip_dict_basic` 与 `image_clip_dict_runtime` DXF 样例已加入 Tests 目录，相关 Golden JSON 由 `cargo test --test dxf_loader` 生成，测试覆盖了字典/Reactor/变量的解析路径。
