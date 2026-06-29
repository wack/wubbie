use std::sync::Arc;

use salvo::oapi::extract::JsonBody;
use salvo::prelude::*;

use crate::domain::ItemName;
use crate::repos::ItemStore;
use crate::services::{ItemService, create_item_service};
use crate::views::{CreateItemFailure, CreateItemRequest, CreateItemResponse};

/// Create a new item
#[endpoint(
    tags("Items"),
    summary = "Create a new item",
    responses(
        (status_code = 200, description = "Item created successfully", body = CreateItemResponse),
        (status_code = 400, description = "Invalid request", body = CreateItemFailure),
        (status_code = 500, description = "Internal server error", body = CreateItemFailure),
    )
)]
pub async fn create_item(
    depot: &mut Depot,
    body: JsonBody<CreateItemRequest>,
) -> Json<Result<CreateItemResponse, CreateItemFailure>> {
    // Get the store from depot (injected by ItemStoreMiddleware)
    let store = match depot.obtain::<Arc<dyn ItemStore>>() {
        Ok(store) => store.clone(),
        Err(_) => {
            return Json(Err(CreateItemFailure::Internal(
                "Store not available".to_string(),
            )));
        }
    };

    // Validate and create the name
    let name = match ItemName::try_new(&body.name) {
        Ok(name) => name,
        Err(_) => {
            return Json(Err(CreateItemFailure::InvalidRequest(
                "Invalid name: must be 1-255 characters".to_string(),
            )));
        }
    };

    // Create the item using the service
    let service = create_item_service(store);
    match service.create_item(name, body.description.clone()).await {
        Ok(metadata) => Json(Ok(CreateItemResponse::from(metadata))),
        Err(_) => Json(Err(CreateItemFailure::Internal(
            "Failed to create item".to_string(),
        ))),
    }
}
