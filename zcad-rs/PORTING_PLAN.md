# ZCAD Rust 版移植计划

## 阶段 0：调研与目标冻结（完成）
- ✅ 核心能力范围冻结：DXF 解析与写出、几何实体（线段、圆弧、多段线、文字）、选择与对象捕捉、电气拓展组件。
- ✅ 依赖映射：Lazarus/LCL → `bevy`（UI 结合 `bevy_egui`）；OpenGL/GDI → `bevy` 默认 `wgpu` 渲染；Pascal 几何工具 → `glam 0.30`；Makefile + typeexporter → Cargo build 脚本与运行时资源打包。
- ✅ 目标平台对齐：Windows 10+、Ubuntu 22.04+、macOS 13+，优先 x86_64；ARM64 macOS 作为可选验证。
- ✅ 首个里程碑：Rust 版本启动后可加载 DXF、构建内部文档模型并通过 bevy + egui 提供最小可交互视图（缩放/平移、实体清单）。

## 阶段 1：架构设计与基础设施（完成）
- ✅ 工作区拆分：新增 `zcad-core`、`zcad-engine`、`zcad-io`、`zcad-frontend`、`zcad-app`，依赖关系已在 `docs/architecture.md` 描述。
- ✅ 引入基础依赖：`glam 0.30`（双精度几何）、`thiserror`、可选 `bevy 0.17.2` + `bevy_egui 0.38.0`，并为前端提供特性开关。
- ✅ 开发工具链：提供 `Makefile` 封装 `cargo fmt/clippy/test`，并计划在后续 CI 中直接复用。
- ✅ 日志 & 配置：新增 `zcad-config` crate（`toml` + `serde` + `thiserror`），应用层使用 `tracing`/`tracing-subscriber` 初始化日志，支持 `ZCAD_CONFIG` 覆盖与默认 `config/default.toml`。

## 阶段 2：几何与数据核心迁移（进行中）
- 以 `core` crate 实现基础实体、矩阵/向量运算，可复用 `nalgebra` 或 `glam`。  
  - ✅ 已建模线段/圆/圆弧/多段线/文字等基础实体，提供 `Document` 构建接口与示例。
- 重写 DXF/自定义格式读取器，逐步覆盖实体类型；引入 golden data 与 Pascal 版本对照测试。  
  - ✅ `zcad-io` 提供黄金快照框架，自动校验 `basic_entities` / `bulge_polyline` / `mtext` / `block_*` 样例并生成差异文件。
- 构建场景图和对象生命周期管理（ID、图层、属性）。  
  - 🛠️ 继续扩展场景选择集、视口状态与更多实体支持。
- 新增《阶段 2 实体覆盖矩阵》，用于跟踪缺失实体与黄金数据计划。参见 `docs/stage2_entity_matrix.md`。
- ✅ 已补齐 Hatch（含椭圆/样条边界、渐变填充）、Dimension（线性/角度/直径/半径/三点角度）以及 Spline（控制/拟合点与节点数据）的数据结构、DXF 解析与黄金样例。
- 🔜 下一步聚焦项：  
  - ✅ Leader/MLeader 扩展缩放、狗腿、落脚间隙等关键参数，新增 `mleader_block*.dxf` / `mleader_block_connections.dxf` 黄金样例并更新 CLI/前端渲染逻辑
  - ✅ Raster/图像实体在缺失资源时提供占位纹理与日志提示，补充 `image_missing_file.dxf` 黄金样例验证资源定位与缓存回退
  - ✅ 3DFACE 已接入 `zcad-core`/`zcad-io`/CLI/Bevy，新增 `face3d_basic.dxf` 等黄金样例并输出隐藏边/法向诊断，支持 POLYFACE / POLYGON MESH（含 wrap）拆解及 Bevy 法向着色
  - 🛠️ 在 Bevy 前端完成 Hatch 渐变与 Dimension 类型的端到端渲染验证并记录可视差异
    - ✅ CLI/前端新增 Hatch 与 Dimension 包围盒等诊断信息，便于与 Pascal 版本比对

## 阶段 3：渲染与交互原型（4-6 周）
- 选定渲染栈（`wgpu` 或 `bevy`），完成窗口、输入、视图控制最小闭环。
- 渲染基础实体，支持缩放、平移、栅格捕捉等基础交互。
- 提供命令系统基础设施，支持脚本化扩展。
  - ✅ CLI 与 Bevy 前端复用统一加载器和命令总线，Bevy 版本已渲染 Hatch 渐变并支持平移/缩放与键盘命令触发。
  - ✅ Bevy 前端补齐椭圆采样与单行/多行文字渲染，自动加载内置字体资源。
  - ✅ 块参照在 Bevy 前端完成基础预览（线稿 + 填充 + 属性对齐），材质/字体资源按资源池复用。
  - ✅ Hatch 渐变 shift 与单色模式已通过单元测试覆盖，降低回归风险。
  - ✅ `bevy_app` 适配 Bevy 0.17 API（Mesh2d、键盘输入、事件 Reader 变更），并恢复特性测试。
  - ✅ Raster 图像渲染支持位图加载、矩形/多边形裁剪与边框绘制，缺失资源时提供日志提示。

## 阶段 4：工具链与扩展功能（4 周）
- 迁移运行时环境生成逻辑（替代 `typeexporter`），统一资源打包与路径管理。
  - ✅ 新增 `resources.runtime_root` / `auto_copy_runtime` 配置，`zcad-app` 自动从 `environment/runtimefiles` 复制运行时资源
- 补全配置、国际化、插件接口，并评估电气扩展所需的专用模块。
- 建立脚本/插件桥接（Lua/Python/WASM）的技术验证。

## 阶段 5：测试、性能与验证（持续迭代）
- 建立单元、集成、端到端测试；采用基准测试比较 Pascal 版与 Rust 版性能。
- 在 Windows/Linux/macOS 上完成交叉验证，记录差异并迭代修复。
- 制定发布策略：Rust 版 beta 与 Pascal 版并行维护，准备迁移文档与用户指南。
