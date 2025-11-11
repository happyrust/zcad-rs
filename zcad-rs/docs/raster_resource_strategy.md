# Raster 图像资源定位与缓存方案

本文档基于阶段 2 的 Raster/Image 实体调研，给出 Rust 版 ZCAD 对 DXF 外部图像资源的管理与加载策略。目标是确保 `IMAGE` 实体在不同运行环境下都能可靠定位位图文件，并在前端渲染层实现高效复用。

## 现状回顾

- `zcad-core` 已提供 `RasterImage`、`RasterImageDefinition` 以及显示选项等数据结构。
- `zcad-io` 能够解析 `IMAGE` 与 `IMAGEDEF` 基础字段，并建立句柄关联。
- 暂未实现图像文件的实际访问、缓存或渲染加载；运行时也缺少集中式资源管理约定。

## 资源定位策略

图像定义中的 `file_path` 可能是绝对路径、相对路径或基于外部参照目录的路径。推荐按照以下优先级查找：

1. **文档所在目录**：以 DXF 文件目录为根解析相对路径（`Path::parent()` + `file_path`）。
2. **显式资源根**：支持通过配置项（`resources.image_roots`）或环境变量 `ZCAD_IMAGE_ROOTS` 指定额外搜索目录，按声明顺序查找。
3. **项目运行时资源包**：若存在打包好的纹理目录（例如 `environment/runtimefiles/.../images`），在初始化阶段注入到搜索列表。
4. **绝对路径兜底**：若 `file_path` 已是绝对路径，则直接尝试访问；如缺失需记录警告。

> 建议解析层将最终解析出的绝对路径存入 `RasterImageDefinition` 的附加字段（后续可引入 `ResolvedDiskPath`），避免重复解析。

## 缓存与句柄管理

- 在 `zcad-io` 层为 `RasterImageDefinition` 增加可选的 `resolved_path: Option<PathBuf>` 字段，解析完成后存储第一次找到的绝对路径。
- 前端渲染 (`zcad-frontend`) 使用路径字符串作为纹理缓存键，并在加载流程结束后根据本次文档需求回收未使用的句柄，避免同一文件重复加载或长期占用内存。
  - 若位图文件缺失或加载失败，前端会在日志中记录详细信息（包括 `handle`、原始路径与解析策略），同时跳过纹理渲染并保留边框提示。

## 配置与可扩展性

- 在 `zcad-config` 添加 `resources.image_roots: Vec<PathBuf>`，并支持通过命令行或环境变量设置。默认情况下为空列表，仅使用 DXF 相对路径。
- 允许在未来扩展为平台特定路径（例如 macOS bundle 资源、Windows 可执行目录等），配置层仅负责提供候选目录。

## 实现路线图

1. **解析层增强**（阶段 2 后续迭代）  
   - 在 `RasterImageDefinition` 增加 `resolved_path` 字段（可选）。  
   - `parse_image_def` 结束后不立即查找文件，仅保留原始 `file_path`，由上层在加载阶段解析。  

2. **资源定位器模块**（阶段 3 准备）  
   - ✅ `zcad-frontend::resource_locator` 已实现 `ImageLocator`：根据 DXF 基目录、配置项与 `ZCAD_IMAGE_ROOTS` 查找位图。  
   - ✅ 文档加载阶段会记录解析结果并更新 `RasterImageDefinition::resolved_path`，便于前端后续复用。  

3. **渲染缓存与占位纹理**（阶段 3）  
   - 构建纹理缓存结构，按 resolved 路径去重加载。  
   - 当文件缺失时创建统一的占位纹理，并向 UI 提示。  

4. **打包与部署支持**（阶段 4）  
   - 将图像资源打包至运行时目录，并在配置中自动注入对应搜索根。  
   - 针对安装包或 AppBundle 设定默认的资源路径。

## 后续工作

- 在 `docs/stage2_entity_matrix.md` 与 `PORTING_PLAN.md` 中同步资源策略实施计划。  
- 为解析失败的情形设计测试样例（包含缺失文件、错误路径、平台差异等），保障回退逻辑的稳定性。  
- 与前端团队协作，验证 Bevy/WGPU 的纹理生命周期与缓存复用策略。
