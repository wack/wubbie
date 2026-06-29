use salvo::oapi::ToSchema;
use serde::Serialize;

use crate::domain::ItemMetadata;

/// Single item in the list response
#[derive(Debug, Serialize, ToSchema)]
pub struct ItemSummary {
    /// The unique identifier for the item
    pub id: String,
    /// The name of the item
    pub name: String,
    /// The description of the item
    pub description: Option<String>,
    /// When the item was created (ISO 8601)
    pub created_at: String,
    /// When the item was last updated (ISO 8601)
    pub updated_at: String,
}

impl From<ItemMetadata> for ItemSummary {
    fn from(metadata: ItemMetadata) -> Self {
        Self {
            id: metadata.id.to_string(),
            name: metadata.name.to_string(),
            description: metadata.description,
            created_at: metadata.created_at.to_rfc3339(),
            updated_at: metadata.updated_at.to_rfc3339(),
        }
    }
}

/// Response containing a list of items
#[derive(Debug, Serialize, ToSchema)]
pub struct ListItemsResponse {
    /// The list of items
    pub items: Vec<ItemSummary>,
}

impl ListItemsResponse {
    pub fn new(items: Vec<ItemMetadata>) -> Self {
        Self {
            items: items.into_iter().map(ItemSummary::from).collect(),
        }
    }
}

/// Error response for list operation
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "error", content = "message")]
pub enum ListItemsFailure {
    /// Internal server error
    Internal(String),
}
