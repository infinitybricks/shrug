pub mod openai;
pub mod typesafe;

/// A request parsed into the normalized model, plus the echoed model name.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedRequest {
    pub model: Option<String>,
    pub questions: Vec<crate::model::Question>,
}
