# ZCAD Rust 架构草案（阶段 1）

## 工作区概览
```
rust/
 ├─ Cargo.toml            # Workspace + resolver v2
 ├─ zcad-core/            # 几何与文档模型
 ├─ zcad-engine/          # 场景、命令与运行时状态
 ├─ zcad-io/              # 文件读写门面（DXF 起步）
 ├─ zcad-frontend/        # 前端适配层（CLI、Bevy + egui）
 ├─ zcad-config/          # 配置加载（toml）与默认值
 ├─ zcad-app/             # 可执行入口，后续扩展多二进制
 ├─ config/default.toml   # 默认应用配置，可通过 `ZCAD_CONFIG` 覆盖
 └─ Makefile              # 本地开发脚本（fmt/lint/test/ci）
```

## Crate 职责
- **zcad-core**：数据层。基于 `glam::DVec2` 封装 `Point2`/`Vector2`，定义 `Document`、`Layer`、`Entity` 等结构，并派生 `serde` 以支持序列化。
- **zcad-engine**：引擎层。管理 `Document` 实例、封装命令上下文（当前提供 `Scene` 骨架和示例数据填充），负责选择集、视口状态等运行时逻辑。
- **zcad-io**：I/O 门面。定义 `DocumentLoader`/`DocumentSaver` trait，并提供 `DxfFacade` 占位实现，统一错误类型 `IoError`（`thiserror`）。
- **zcad-frontend**：界面与交互。CLI 默认启用，Bevy + `bevy_egui` 通过 `bevy_app` 特性激活；提供公共错误类型 `FrontendError` 和启动函数封装，并在 CLI/Bevy 中共享 `Scene::load_document` 与命令通道。Bevy 原型已支持线段/多段线/Hatch、椭圆采样、块参照线稿与填充预览以及单行/多行文字/属性对齐渲染。
- **zcad-engine::command**：定义命令总线与基础命令（聚焦/清选），并提供 `CommandContext` 协调前端对 `Scene` 的操作。
- **zcad-config**：负责读取 `toml` 配置文件，支持环境变量覆盖，并提供默认 `FrontendMode` / 日志等级等设置。
- **zcad-app**：顶层可执行文件，目前解析 `--bevy`/`--cli`/`--config` 参数；初始化 `tracing` 日志后，再根据配置选择 CLI 或 Bevy 前端。

## 依赖与特性
- `glam 0.30`：采用 `DVec2` 保持与 Pascal 版双精度一致，启用 `serde` 特性。
- `bevy 0.17.2` + `bevy_egui 0.38.0`：通过 `zcad-frontend` 的 `bevy_app` 特性按需拉取，避免在 CLI 模式下的构建开销。
- `thiserror`：在引擎、I/O、前端、配置 crate 中统一错误定义。
- `tracing` / `tracing-subscriber`：在应用启动阶段初始化日志，CLI/引擎前端输出运行时信息；后续可复用同一体系记录性能指标。
- `toml` + `serde`：配置解析、黄金样例等都通过 `serde` 序列化，保持与测试数据一致。

## 后续扩展指引
- 引擎层将新增命令系统、选择管理与视口参数缓存，由 `Scene` 提供对 `Document` 的封装。
- I/O 层计划在阶段 2 实现 DXF 解析器，使用黄金样例与 Pascal 版对比测试；`zcad-io/tests/golden.rs` 已提供快照框架。
- 前端层已在阶段 3 接入基础渲染：`bevy_app::launch(title)` 绘制线/圆/多段线与 Hatch 渐变填充，支持鼠标平移/缩放，并通过命令总线与场景交互。
- 统一工具链：`Makefile` 已封装 `fmt`/`lint`/`test`/`ci`，后续 CI 可直接调用；未来增加 `cargo clippy --workspace --all-targets` 与发布流水线。
