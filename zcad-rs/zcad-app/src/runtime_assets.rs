use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use tracing::info;
use zcad_config::ResourceConfig;

pub fn ensure_runtime_assets(config: &ResourceConfig) -> Result<(), RuntimeAssetError> {
    let Some(root) = config.runtime_root.as_ref() else {
        return Ok(());
    };
    let target = normalize_path(root)?;
    if target.exists() {
        return Ok(());
    }
    if !config.auto_copy_runtime {
        return Err(RuntimeAssetError::AutoCopyDisabled { target });
    }
    let source = locate_runtime_source().ok_or(RuntimeAssetError::SourceNotFound)?;
    copy_dir_recursive(&source, &target).map_err(|error| RuntimeAssetError::CopyFailed {
        source: source.clone(),
        target: target.clone(),
        error,
    })?;
    info!(
        source = %source.display(),
        target = %target.display(),
        "已复制运行时资源"
    );
    Ok(())
}

fn normalize_path(path: &Path) -> Result<PathBuf, RuntimeAssetError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        let cwd = env::current_dir().map_err(|error| RuntimeAssetError::Io { error })?;
        Ok(cwd.join(path))
    }
}

fn locate_runtime_source() -> Option<PathBuf> {
    if let Some(env_path) = env::var_os("ZCAD_RUNTIME_SOURCE") {
        let candidate = PathBuf::from(env_path);
        if candidate.is_dir() {
            return Some(candidate);
        }
    }

    let cwd = env::current_dir().ok()?;
    let candidates = [
        cwd.join("environment").join("runtimefiles"),
        cwd.join("..").join("environment").join("runtimefiles"),
    ];
    candidates.into_iter().find(|path| path.is_dir())
}

fn copy_dir_recursive(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = target.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::create_dir_all(dst_path.parent().unwrap_or(target))?;
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[derive(Debug)]
pub enum RuntimeAssetError {
    SourceNotFound,
    AutoCopyDisabled {
        target: PathBuf,
    },
    CopyFailed {
        source: PathBuf,
        target: PathBuf,
        error: std::io::Error,
    },
    Io {
        error: std::io::Error,
    },
}

impl Display for RuntimeAssetError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeAssetError::SourceNotFound => {
                write!(f, "未能找到运行时资源源目录")
            }
            RuntimeAssetError::AutoCopyDisabled { target } => {
                write!(
                    f,
                    "目标 {} 不存在且 auto_copy_runtime 已关闭",
                    target.display()
                )
            }
            RuntimeAssetError::CopyFailed {
                source,
                target,
                error,
            } => {
                write!(
                    f,
                    "复制运行时资源 {} -> {} 失败: {}",
                    source.display(),
                    target.display(),
                    error
                )
            }
            RuntimeAssetError::Io { error } => {
                write!(f, "I/O 错误: {error}")
            }
        }
    }
}

impl Error for RuntimeAssetError {}
