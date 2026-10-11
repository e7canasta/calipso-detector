/// A failure loading or running a model, before it becomes the core's failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ModelError {
    #[error("model load failed: {0}")]
    Load(String),
    #[error("model inference failed: {0}")]
    Inference(String),
}
