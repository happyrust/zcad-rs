# zcad-app 运行指南

`zcad-app` 提供 CLI/Bevy 两种前端模式，并依赖 `runtime-builder` 准备 Pascal 版本的运行时资产。建议通过根目录 `rust/Makefile` 调用，自动完成运行时复制与应用启动。

## 快速开始

```bash
make -C rust app
```

该命令会：

1. 调用 `make -C rust runtime`，执行 `cargo run -p runtime-builder` 将 `environment/runtimefiles` 拷贝到 `rust/runtime/dist/`，并生成 `runtime_manifest.json`。
2. 在运行 `zcad-app` 前确保 `config/default.toml` 中的 `resources.runtime_root` 指向 `runtime/dist`（默认配置已如此设置，且支持 `auto_copy_runtime = true`）。
3. 启动 `zcad-app`，默认以 CLI 模式运行；使用 `APP_ARGS="--bevy"` 可切换到 Bevy 窗口模式。

## 可选参数

- `RUNTIME_PRODUCT`：选择 `zcad` 或 `zcadelectrotech`，默认为 `zcad`。
- `RUNTIME_PLATFORM`：指定额外的平台增量目录（例如 `x86_64-win64`）。不指定则仅复制 `AllCPU-AllOS`。
- `RUNTIME_TARGET`：运行时输出目录，默认 `runtime/dist`。
- `APP_ARGS`：透传给 `zcad-app` 的启动参数，如 `APP_ARGS="--bevy --config custom.toml"`。

示例：为电气版本准备 Windows 运行时并启动 Bevy 窗口：

```bash
make -C rust app RUNTIME_PRODUCT=zcadelectrotech RUNTIME_PLATFORM=x86_64-win64 APP_ARGS="--bevy"
```

## 运行时资源校验

`runtime-builder` 会为输出目录生成 `runtime_manifest.json`（包含文件大小与 SHA256），后续 CI 将该 manifest 用于校验资源缺失情况。可执行以下命令验证当前运行时目录：

```bash
cargo run -p runtime-builder -- verify --manifest rust/runtime/dist/runtime_manifest.json --target rust/runtime/dist
```

命令会对 manifest 中的每个文件进行大小与 SHA256 校验，若存在缺失或内容不一致，会在 stdout 中给出具体条目。
