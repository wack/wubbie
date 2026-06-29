use std::fmt::Debug;
use thiserror::Error;

#[derive(Error)]
#[error("An internal server error has occurred")]
pub struct InternalError(#[from] anyhow::Error);

impl Debug for InternalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self, self.0)
    }
}

impl InternalError {
    pub fn new(err: anyhow::Error) -> Self {
        Self(err)
    }
}

impl Drop for InternalError {
    fn drop(&mut self) {
        tracing::error!(msg = %self.0, "internal error");
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::InternalError;
    use anyhow::anyhow;
    use pretty_assertions::assert_str_eq;

    /// Demonstrate the Forbidden error renders in display format correctly.
    #[test]
    fn internal_error_renders_fmt() {
        let err = InternalError::new(anyhow!("bad token"));
        let expected = "An internal server error has occurred";
        let observed = err.to_string();
        assert_str_eq!(expected, observed);
    }

    /// Demonstrate the Forbidden error renders in debug format correctly.
    #[test]
    fn internal_error_renders_debug() {
        let err = InternalError::new(anyhow!("bad token"));
        let expected = "An internal server error has occurred: bad token";
        let observed = format!("{err:?}");
        assert_str_eq!(expected, observed);
    }
}
