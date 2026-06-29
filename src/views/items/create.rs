use salvo::oapi::ToSchema;
use serde::{Deserialize, Serialize};

use crate::domain::ItemMetadata;

/// Request body for creating a new item
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateItemRequest {
    /// The name of the item (1-255 characters)
    pub name: String,
    /// Optional description for the item
    pub description: Option<String>,
}

/// Response for a successfully created item
#[derive(Debug, Serialize, ToSchema)]
pub struct CreateItemResponse {
    /// The unique identifier for the created item
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

impl From<ItemMetadata> for CreateItemResponse {
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

/// Error response for create operation
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "error", content = "message")]
pub enum CreateItemFailure {
    /// Invalid request parameters
    InvalidRequest(String),
    /// Internal server error
    Internal(String),
}
