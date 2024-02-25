use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum ParseCallbackDataError {
    #[error("unknown callback code `{code}`")]
    UnknownCallbackCode { code: String },
}
