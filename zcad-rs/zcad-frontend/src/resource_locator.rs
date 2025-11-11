use std::env;
use std::path::{Path, PathBuf};

use tracing::{debug, trace};
use zcad_config::AppConfig;
use zcad_core::document::Document;

const IMAGE_ROOTS_ENV: &str = "ZCAD_IMAGE_ROOTS";

pub struct ImageLocator {
    search_roots: Vec<PathBuf>,
}

impl ImageLocator {
    pub fn from_config(base_dir: Option<&Path>, config: &AppConfig) -> Self {
        let mut roots: Vec<PathBuf> = Vec::new();

        if let Some(dir) = base_dir {
            roots.push(dir.to_path_buf());
        }

        roots.extend(
            config
                .resources
                .image_roots
                .iter()
                .cloned()
                .filter(|path| path.is_dir()),
        );

        if let Some(env_paths) = env::var_os(IMAGE_ROOTS_ENV) {
            let splitter = env::split_paths(&env_paths);
            for path in splitter {
                if path.is_dir() {
                    roots.push(path);
                }
            }
        }

        // 去重，保持靠前优先级。
        let mut deduped: Vec<PathBuf> = Vec::new();
        for root in roots {
            if !deduped.iter().any(|existing| existing == &root) {
                deduped.push(root);
            }
        }

        ImageLocator {
            search_roots: deduped,
        }
    }

    pub fn resolve(&self, path_str: &str) -> Option<PathBuf> {
        let raw_path = Path::new(path_str);
        if raw_path.is_absolute() {
            if raw_path.exists() {
                return Some(Self::canonicalize_or_clone(raw_path));
            }
        } else {
            for root in &self.search_roots {
                let candidate = root.join(raw_path);
                trace!(candidate = %candidate.display(), "image locator candidate");
                if candidate.exists() {
                    return Some(Self::canonicalize_or_clone(&candidate));
                }
            }
        }

        if raw_path.is_absolute() {
            debug!(
                path = %raw_path.display(),
                "IMAGE 路径为绝对路径但未找到对应文件"
            );
        }

        None
    }

    fn canonicalize_or_clone(path: &Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    }
}

pub fn apply_image_resolution(document: &mut Document, locator: &ImageLocator) {
    for (_, definition) in document.raster_image_definitions_mut() {
        if definition.resolved_path.is_some() {
            continue;
        }
        if let Some(resolved) = locator.resolve(&definition.file_path) {
            let resolved_str = resolved.to_string_lossy().into_owned();
            definition.resolved_path = Some(resolved_str);
        }
    }
}
