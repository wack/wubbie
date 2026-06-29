use salvo::oapi::ToSchema;
use serde::Serialize;

/// Response for a successful delete operation
#[derive(Debug, Serialize, ToSchema)]
pub struct DeleteItemResponse {
    /// Confirmation message
    pub message: String,
}

impl DeleteItemResponse {
    pub fn new() -> Self {
        Self {
            message: "Item deleted successfully".to_string(),
        }
    }
}

impl Default for DeleteItemResponse {
    fn default() -> Self {
        Self::new()
    }
}

/// Error response for delete operation
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "error", content = "message")]
pub enum DeleteItemFailure {
    /// Invalid item ID format
    InvalidParameter(String),
    /// Internal server error
    Internal(String),
}
