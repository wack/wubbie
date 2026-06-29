use std::fmt::Debug;
use thiserror::Error;

#[derive(Error)]
#[error("The user is forbidden from performing this action")]
pub struct Forbidden(#[from] anyhow::Error);

impl Debug for Forbidden {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self, self.0)
    }
}

impl Forbidden {
    pub fn new(err: anyhow::Error) -> Self {
        Self(err)
    }
}

#[cfg(test)]
mod tests {
    use super::Forbidden;
    use anyhow::anyhow;
    use pretty_assertions::assert_str_eq;

    /// Demonstrate the Forbidden error renders in display format correctly.
    #[test]
    fn forbidden_renders_fmt() {
        let err = Forbidden::new(anyhow!("bad token"));
        let expected = "The user is forbidden from performing this action";
        let observed = err.to_string();
        assert_str_eq!(expected, observed);
    }

    /// Demonstrate the Forbidden error renders in debug format correctly.
    #[test]
    fn forbidden_renders_debug() {
        let err = Forbidden::new(anyhow!("bad token"));
        let expected = "The user is forbidden from performing this action: bad token";
        let observed = format!("{err:?}");
        assert_str_eq!(expected, observed);
    }
}
