use std::path::PathBuf;

use tracing::{error, info, warn};
use tracing_subscriber::{EnvFilter, fmt};
use zcad_config::{AppConfig, ConfigError, FrontendMode};

mod runtime_assets;

fn main() {
    let mut args = std::env::args().skip(1);
    let mut override_mode: Option<FrontendMode> = None;
    let mut config_override: Option<PathBuf> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bevy" => override_mode = Some(FrontendMode::Bevy),
            "--cli" => override_mode = Some(FrontendMode::Cli),
            "--config" => {
                let Some(path) = args.next() else {
                    eprintln!("`--config` 需要提供配置文件路径");
                    std::process::exit(1);
                };
                config_override = Some(PathBuf::from(path));
            }
            other => {
                eprintln!("未知参数：{other}");
                std::process::exit(1);
            }
        }
    }

    let config = load_configuration(config_override);
    init_logging(&config);
    info!("启动 ZCAD Rust 应用");

    if let Err(err) = runtime_assets::ensure_runtime_assets(&config.resources) {
        warn!(error = %err, "无法准备运行时资源");
    }

    let mode = override_mode.unwrap_or(config.frontend.default_mode);
    match mode {
        FrontendMode::Bevy => {
            info!("以 Bevy 模式启动");
            if let Err(err) =
                zcad_frontend::launch_bevy_desktop_with_title(&config.frontend.bevy_window_title)
            {
                error!(error = %err, "无法启动 Bevy 前端");
                std::process::exit(1);
            }
        }
        FrontendMode::Cli => {
            info!("以 CLI 模式启动");
            if let Err(err) = zcad_frontend::run_cli_demo() {
                error!(error = %err, "执行 CLI 演示失败");
                std::process::exit(1);
            }
        }
    }
}

fn load_configuration(override_path: Option<PathBuf>) -> AppConfig {
    match override_path {
        Some(path) => AppConfig::from_file(&path).unwrap_or_else(|err| {
            warn!(path = %path.display(), error = %err, "加载指定配置失败，使用默认配置");
            AppConfig::default()
        }),
        None => match AppConfig::discover() {
            Ok(cfg) => cfg,
            Err(err) => {
                match &err {
                    ConfigError::Io { path, .. } | ConfigError::Parse { path, .. } => {
                        warn!(path = %path.display(), error = %err, "加载默认配置失败，使用内建默认值");
                    }
                    ConfigError::Context { .. } => {
                        warn!(error = %err, "加载默认配置失败，使用内建默认值");
                    }
                }
                AppConfig::default()
            }
        },
    }
}

fn init_logging(config: &AppConfig) {
    let filter =
        EnvFilter::try_new(config.logging.level.clone()).unwrap_or_else(|_| EnvFilter::new("info"));
    let subscriber = fmt().with_env_filter(filter);
    if subscriber.try_init().is_err() {
        // 已初始化，忽略
    }
}
