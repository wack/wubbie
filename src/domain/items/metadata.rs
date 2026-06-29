use chrono::{DateTime, Utc};
use serde::Serialize;

use super::{ItemId, ItemName};

/// Public metadata for an Item.
///
/// This struct contains all the information about an item that can be
/// safely exposed through the API.
#[derive(Clone, Debug, Serialize)]
pub struct ItemMetadata {
    /// The unique identifier for the item
    pub id: ItemId,
    /// The display name of the item
    pub name: ItemName,
    /// Optional description of the item
    pub description: Option<String>,
    /// When the item was created
    pub created_at: DateTime<Utc>,
    /// When the item was last updated
    pub updated_at: DateTime<Utc>,
}

impl ItemMetadata {
    /// Creates a new ItemMetadata instance
    pub fn new(
        id: ItemId,
        name: ItemName,
        description: Option<String>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            name,
            description,
            created_at,
            updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_creation() {
        let now = Utc::now();
        let metadata = ItemMetadata::new(
            ItemId::random(),
            ItemName::try_new("Test Item").unwrap(),
            Some("A test description".to_string()),
            now,
            now,
        );

        assert_eq!(metadata.name.as_ref(), "Test Item");
        assert_eq!(metadata.description, Some("A test description".to_string()));
    }
}
