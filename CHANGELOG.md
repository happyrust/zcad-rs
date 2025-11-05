
# 更改日志

## [未发布] - 2025-11-05

### 添加
+ 添加了 TEXT 命令的操作数
+ Rust 子项目：初始化 `zcad-core`、`zcad-engine`、`zcad-io`、`zcad-frontend`、`zcad-app` 工作区骨架
+ Rust 子项目：引入 DXF 解析占位实现与 covering 集成测试数据
+ Rust 子项目：Bevy 原型新增椭圆与单行/多行文字渲染支持
+ Rust 子项目：Bevy 原型渲染块参照（含填充与属性对齐）并复用基础材质
+ Rust 子项目：新增 `block_hatch` DXF 黄金样例覆盖块参照填充
+ Rust 子项目：Bevy 前端新增 Hatch 渐变 shift 与单色覆盖单元测试
+ Rust 子项目：引入 LEADER/MULTILEADER 最小数据结构与 DXF 解析，并补充黄金测试
+ Rust 子项目：实现 IMAGE/IMAGEDEF 最小解析，新增 `image_basic` 黄金样例覆盖 Raster 实体
+ Rust 子项目：整理 Raster/Image 实体调研并记录阶段 2 规划更新
+ Rust 子项目：新增 `image_clip_polygon.dxf` 黄金样例，为 IMAGE 多边形裁剪解析与渲染测试提供基准
+ Rust 子项目：IMAGE 解析支持矩形和多边形裁剪，并覆盖黄金测试与单元验证
+ Rust 子项目：`zcad-core` 增加 Raster 图像字典与变量结构，`zcad-io` 解析 `ACAD_IMAGE_DICT` / `RASTERVARIABLES` / `IMAGEDEF_REACTOR` 并刷新黄金测试
+ Rust 子项目：Bevy 前端渲染 Raster 图像，支持矩形/多边形裁剪与边框显示

### 更改
* 修复了块插入的 3D 变换
* 更改或添加了绘制时的减退算法，提高了绘制速度
* Rust 子项目：核心几何改用 `glam` 双精度向量，前端支持可选 `bevy` + `bevy_egui`
* Rust 子项目：整理阶段 2 实体覆盖矩阵并规划缺口测试
* Rust 子项目：Bevy 渐变填充现在兼容 AutoCAD True Color、SHIFT 参数，并对角度零向量进行保护
* Rust 子项目：Bevy 前端适配 0.17 API（Mesh2d、MessageReader、键盘输入升级）
* Rust 子项目：复盘阶段 2 行动项并确认 Leader/MLeader、Raster 与对象捕捉为下一批重点
* Rust 子项目：更新移植开发计划，明确 Leader/MLeader 建模与 Raster 预研的下一步行动
* Rust 子项目：补充 Raster 图像资源定位与缓存策略文档，规划阶段 3 执行计划
* Rust 子项目：盘点 IMAGE 裁剪与字典解析需求，扩充裁剪落地计划并细化解析状态机与数据结构安排

### 修复
* Rust 子项目：修正 DXF 属性解析，支持行距参数与基本格式转义

## [0.9.17.0] - 2025-05-04

更新时应删除以前版本的配置文件

### 添加
+ 在 MTEXT 中跳过文本格式标记

### 更改

### 修复
* 修复了在 qt 版本中打开或创建第二个草图时出现的问题

## [0.9.16.2] - 2025-01-26

### 添加

### 更改

### 修复
