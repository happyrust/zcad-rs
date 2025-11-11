use std::env;
use std::path::PathBuf;

use tracing::{info, warn};
use zcad_config::AppConfig;
use zcad_engine::scene::{DemoEntities, Scene};
use zcad_io::{DocumentLoader, DxfFacade};

use crate::resource_locator::{ImageLocator, apply_image_resolution};

/// 文档来源，便于前端呈现加载信息。
#[derive(Debug, Clone)]
pub enum DocumentSource {
    Dxf(PathBuf),
    Demo,
}

/// 统一封装加载后的场景与元信息。
#[derive(Debug)]
pub struct LoadedScene {
    pub scene: Scene,
    pub source: DocumentSource,
    pub demo_entities: Option<DemoEntities>,
}

/// 从环境变量 `ZCAD_CLI_SAMPLE_DXF` 指定的路径加载 DXF，
/// 若失败则回退到内置示例。
pub fn load_scene_from_env_or_demo() -> LoadedScene {
    let mut scene = Scene::new();
    let config = load_app_config();
    if let Some(path) = env::var_os("ZCAD_CLI_SAMPLE_DXF") {
        let path = PathBuf::from(path);
        let loader = DxfFacade::new();
        match loader.load(&path) {
            Ok(mut document) => {
                info!(path = %path.display(), "从 DXF 加载文档成功");
                apply_image_resolution(
                    &mut document,
                    &ImageLocator::from_config(path.parent(), &config),
                );
                scene.load_document(document);
                return LoadedScene {
                    scene,
                    source: DocumentSource::Dxf(path),
                    demo_entities: None,
                };
            }
            Err(err) => {
                warn!(path = %path.display(), error = %err, "加载 DXF 失败，回退到内置示例");
            }
        }
    }

    let demo_entities = scene.populate_demo();
    // 保持与 CLI 旧逻辑一致，选中圆与文字后聚焦。
    let _ = scene.select(demo_entities.circle);
    let _ = scene.select(demo_entities.label);
    scene.focus_on_selection();

    LoadedScene {
        scene,
        source: DocumentSource::Demo,
        demo_entities: Some(demo_entities),
    }
}

fn load_app_config() -> AppConfig {
    match AppConfig::discover() {
        Ok(cfg) => cfg,
        Err(err) => {
            warn!(error = %err, "读取配置失败，使用默认配置");
            AppConfig::default()
        }
    }
}
