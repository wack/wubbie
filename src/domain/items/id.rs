use nutype::nutype;
use uuid::Uuid;

/// Unique identifier for an Item.
///
/// This is the public-facing ID (UUID) used in API responses.
#[nutype(
    derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        Serialize,
        Deserialize,
        Display,
        FromStr,
        AsRef
    ),
    new_unchecked
)]
pub struct ItemId(Uuid);

impl ItemId {
    /// Creates a new random ItemId
    pub fn random() -> Self {
        Self::new(Uuid::new_v4())
    }
}

impl Default for ItemId {
    fn default() -> Self {
        Self::random()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_id_creation() {
        let id = ItemId::random();
        let id_str = id.to_string();
        assert!(!id_str.is_empty());
    }

    #[test]
    fn test_item_id_from_str() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let id: ItemId = uuid_str.parse().unwrap();
        assert_eq!(id.to_string(), uuid_str);
    }
}
