use salvo::oapi::ToSchema;
use serde::{Deserialize, Serialize};

use crate::domain::ItemMetadata;

/// Request body for updating an item
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateItemRequest {
    /// New name for the item (1-255 characters)
    pub name: Option<String>,
    /// New description for the item (null to clear)
    pub description: Option<Option<String>>,
}

/// Response for a successfully updated item
#[derive(Debug, Serialize, ToSchema)]
pub struct UpdateItemResponse {
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

impl From<ItemMetadata> for UpdateItemResponse {
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

/// Error response for update operation
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "error", content = "message")]
pub enum UpdateItemFailure {
    /// Item not found
    NotFound(String),
    /// Invalid request parameters
    InvalidRequest(String),
    /// Invalid item ID format
    InvalidParameter(String),
    /// Internal server error
    Internal(String),
}
