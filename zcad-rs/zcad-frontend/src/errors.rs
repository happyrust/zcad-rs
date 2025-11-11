use thiserror::Error;

#[derive(Debug, Error)]
pub enum FrontendError {
    #[error("Bevy 前端未启用，请使用 `--features bevy_app` 编译")]
    BevyFeatureDisabled,
}
