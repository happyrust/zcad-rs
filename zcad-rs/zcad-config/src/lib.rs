use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

/// 应用配置的根结构。
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub frontend: FrontendConfig,
    #[serde(default)]
    pub resources: ResourceConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            logging: LoggingConfig::default(),
            frontend: FrontendConfig::default(),
            resources: ResourceConfig::default(),
        }
    }
}

impl AppConfig {
    /// 从显式路径加载配置。
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        toml::from_str(&content).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })
    }

    /// 自动发现配置文件：优先读取环境变量 `ZCAD_CONFIG`，否则寻找 `./config/default.toml`。
    /// 若文件缺失，则返回默认配置。
    pub fn discover() -> Result<Self, ConfigError> {
        if let Some(path) = env::var_os("ZCAD_CONFIG") {
            return Self::from_file(PathBuf::from(path));
        }

        let default_path = env::current_dir()
            .map(|dir| dir.join("config").join("default.toml"))
            .map_err(|source| ConfigError::Context {
                message: "获取当前工作目录失败".to_string(),
                source,
            })?;

        if default_path.exists() {
            Self::from_file(default_path)
        } else {
            Ok(Self::default())
        }
    }
}

/// 日志配置，支持设置默认等级。
#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "LoggingConfig::default_level")]
    pub level: String,
}

impl LoggingConfig {
    fn default_level() -> String {
        "info".to_string()
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: Self::default_level(),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrontendMode {
    Cli,
    Bevy,
}

impl Default for FrontendMode {
    fn default() -> Self {
        FrontendMode::Cli
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct FrontendConfig {
    #[serde(default)]
    pub default_mode: FrontendMode,
    #[serde(default = "FrontendConfig::default_window_title")]
    pub bevy_window_title: String,
}

impl FrontendConfig {
    fn default_window_title() -> String {
        "ZCAD Rust Preview".to_string()
    }
}

impl Default for FrontendConfig {
    fn default() -> Self {
        Self {
            default_mode: FrontendMode::default(),
            bevy_window_title: Self::default_window_title(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResourceConfig {
    #[serde(default)]
    pub image_roots: Vec<PathBuf>,
    #[serde(default)]
    pub runtime_root: Option<PathBuf>,
    #[serde(default = "ResourceConfig::default_auto_copy")]
    pub auto_copy_runtime: bool,
}

impl ResourceConfig {
    fn default_auto_copy() -> bool {
        true
    }
}

impl Default for ResourceConfig {
    fn default() -> Self {
        Self {
            image_roots: Vec::new(),
            runtime_root: None,
            auto_copy_runtime: true,
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("读取配置文件 {path:?} 失败: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("解析配置文件 {path:?} 失败: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("{message}")]
    Context {
        message: String,
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn defaults_are_returned_when_file_missing() {
        let cfg = AppConfig::discover().expect("discover should succeed");
        assert_eq!(cfg.logging.level, "info");
        matches!(cfg.frontend.default_mode, FrontendMode::Cli);
        assert!(cfg.resources.image_roots.is_empty());
        assert!(cfg.resources.runtime_root.is_none());
        assert!(cfg.resources.auto_copy_runtime);
    }

    #[test]
    fn load_from_temp_file() {
        let mut file = tempfile::NamedTempFile::new().expect("create temp file");
        writeln!(
            file,
            r#"
            [logging]
            level = "debug"

            [frontend]
            default_mode = "bevy"
            bevy_window_title = "Custom"

            [resources]
            image_roots = ["../assets", "../textures"]
            runtime_root = "../runtime"
            auto_copy_runtime = false
            "#
        )
        .unwrap();

        let cfg = AppConfig::from_file(file.path()).expect("load config");
        assert_eq!(cfg.logging.level, "debug");
        matches!(cfg.frontend.default_mode, FrontendMode::Bevy);
        assert_eq!(cfg.frontend.bevy_window_title, "Custom");
        assert_eq!(cfg.resources.image_roots.len(), 2);
        assert_eq!(
            cfg.resources
                .runtime_root
                .as_deref()
                .map(|p| p.to_string_lossy().to_string()),
            Some("../runtime".to_string())
        );
        assert!(!cfg.resources.auto_copy_runtime);
    }
}
