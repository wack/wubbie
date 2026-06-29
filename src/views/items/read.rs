use salvo::oapi::ToSchema;
use serde::Serialize;

use crate::domain::ItemMetadata;

/// Response for reading a single item
#[derive(Debug, Serialize, ToSchema)]
pub struct ReadItemResponse {
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

impl From<ItemMetadata> for ReadItemResponse {
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

/// Error response for read operation
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "error", content = "message")]
pub enum ReadItemFailure {
    /// Item not found
    NotFound(String),
    /// Invalid item ID format
    InvalidParameter(String),
    /// Internal server error
    Internal(String),
}
