use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    BadRequest,
    PayloadTooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShrugError {
    pub kind: ErrorKind,
    pub message: String,
}

impl ShrugError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::BadRequest,
            message: msg.into(),
        }
    }
    pub fn payload_too_large(msg: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::PayloadTooLarge,
            message: msg.into(),
        }
    }

    pub fn http_status(&self) -> u16 {
        match self.kind {
            ErrorKind::BadRequest => 400,
            ErrorKind::PayloadTooLarge => 413,
        }
    }

    pub fn error_type(&self) -> &'static str {
        match self.kind {
            ErrorKind::BadRequest => "invalid_request_error",
            ErrorKind::PayloadTooLarge => "payload_too_large",
        }
    }
}

impl fmt::Display for ShrugError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.error_type(), self.message)
    }
}

impl std::error::Error for ShrugError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_and_type_mapping() {
        assert_eq!(ShrugError::bad_request("x").http_status(), 400);
        assert_eq!(ShrugError::payload_too_large("x").http_status(), 413);
        assert_eq!(
            ShrugError::bad_request("x").error_type(),
            "invalid_request_error"
        );
        assert_eq!(ShrugError::bad_request("boom").message, "boom");
    }
}
