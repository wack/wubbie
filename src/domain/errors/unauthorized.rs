#![allow(deprecated)]

use std::fmt::Debug;

#[allow(unused)]
#[deprecated = "Only the middleware should return this status code."]
#[derive(thiserror::Error)]
#[error("The user is not authorized to perform this action")]
pub struct NotAuthorized(#[from] anyhow::Error);

impl NotAuthorized {
    #[cfg(test)]
    pub fn new(err: anyhow::Error) -> Self {
        Self(err)
    }
}

impl Debug for NotAuthorized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self, self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::NotAuthorized;
    use anyhow::anyhow;
    use pretty_assertions::assert_str_eq;

    /// Demonstrate the Forbidden error renders in display format correctly.
    #[test]
    fn not_authorized_renders_fmt() {
        let err = NotAuthorized::new(anyhow!("bad token"));
        let expected = "The user is not authorized to perform this action";
        let observed = err.to_string();
        assert_str_eq!(expected, observed);
    }

    /// Demonstrate the Forbidden error renders in debug format correctly.
    #[test]
    fn not_authorized_renders_debug() {
        let err = NotAuthorized::new(anyhow!("bad token"));
        let expected = "The user is not authorized to perform this action: bad token";
        let observed = format!("{err:?}");
        assert_str_eq!(expected, observed);
    }
}
