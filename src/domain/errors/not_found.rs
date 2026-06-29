use std::fmt::Debug;

#[allow(deprecated)]
use super::forbidden::Forbidden;

#[derive(thiserror::Error)]
#[error("The requested object or path was not found")]
pub struct NotFound(#[from] anyhow::Error);

impl Debug for NotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self, self.0)
    }
}

/// When users aren't authorized to perform an action, we create
/// a Forbidden error, per HTTP's specification. However, we usually
/// want to return a 404 instead so as not to leak private information
/// about a resource this user doesn't have access to. This type
/// convertion makes it easy to return a 404 while still
/// tracking the providence of errors.
#[allow(deprecated)]
impl From<Forbidden> for NotFound {
    fn from(err: Forbidden) -> Self {
        Self::from(anyhow::Error::from(err))
    }
}

impl NotFound {
    pub fn new(err: anyhow::Error) -> Self {
        Self(err)
    }
}

#[cfg(test)]
mod tests {
    use super::NotFound;
    use anyhow::anyhow;
    use pretty_assertions::assert_str_eq;

    /// Demonstrate the Forbidden error renders in display format correctly.
    #[test]
    fn not_found_renders_fmt() {
        let err = NotFound::new(anyhow!("no such user"));
        let expected = "The requested object or path was not found";
        let observed = err.to_string();
        assert_str_eq!(expected, observed);
    }

    /// Demonstrate the Forbidden error renders in debug format correctly.
    #[test]
    fn not_found_renders_debug() {
        let err = NotFound::new(anyhow!("no such user"));
        println!("What the fuck is happening");
        let expected = "The requested object or path was not found: no such user";
        let observed = format!("{err:?}");
        println!("Here's my observed error:");
        println!("{observed:?}");
        assert_str_eq!(expected, observed);
    }
}
