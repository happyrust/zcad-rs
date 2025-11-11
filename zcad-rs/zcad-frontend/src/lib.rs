pub mod cli;
pub mod errors;
pub mod loader;
pub mod resource_locator;

#[cfg(feature = "bevy_app")]
pub mod bevy_app;

use errors::FrontendError;
use tracing::info;

/// 启动 CLI 演示或返回错误。
pub fn run_cli_demo() -> Result<(), FrontendError> {
    info!("启动 CLI 演示前端");
    cli::run_demo();
    Ok(())
}

/// 启动 Bevy + egui 桌面前端，若未启用 `bevy_app` 特性则返回错误。
pub fn launch_bevy_desktop() -> Result<(), FrontendError> {
    launch_bevy_desktop_with_title("ZCAD Rust Preview")
}

/// 允许自定义窗口标题的 Bevy 前端启动。
pub fn launch_bevy_desktop_with_title(title: &str) -> Result<(), FrontendError> {
    #[cfg(feature = "bevy_app")]
    {
        info!(title, "启动 Bevy 桌面前端");
        bevy_app::launch(title);
        Ok(())
    }
    #[cfg(not(feature = "bevy_app"))]
    {
        let _ = title;
        Err(FrontendError::BevyFeatureDisabled)
    }
}
