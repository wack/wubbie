use nutype::nutype;

/// Display name for an Item.
///
/// Must be between 1 and 255 characters, trimmed of whitespace.
#[nutype(
    sanitize(trim),
    validate(not_empty, len_char_max = 255),
    derive(
        Clone,
        Debug,
        PartialEq,
        Eq,
        Hash,
        Serialize,
        Deserialize,
        Display,
        AsRef
    )
)]
pub struct ItemName(String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_name() {
        let name = ItemName::try_new("My Item").unwrap();
        assert_eq!(name.as_ref(), "My Item");
    }

    #[test]
    fn test_trimmed_name() {
        let name = ItemName::try_new("  My Item  ").unwrap();
        assert_eq!(name.as_ref(), "My Item");
    }

    #[test]
    fn test_empty_name_rejected() {
        let result = ItemName::try_new("");
        assert!(result.is_err());
    }

    #[test]
    fn test_whitespace_only_rejected() {
        let result = ItemName::try_new("   ");
        assert!(result.is_err());
    }

    #[test]
    fn test_too_long_name_rejected() {
        let long_name = "a".repeat(256);
        let result = ItemName::try_new(long_name);
        assert!(result.is_err());
    }
}
