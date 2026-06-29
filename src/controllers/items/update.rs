use std::sync::Arc;

use salvo::oapi::extract::JsonBody;
use salvo::prelude::*;

use crate::domain::{ItemId, ItemName};
use crate::repos::ItemStore;
use crate::services::{ItemService, create_item_service, items::ServiceError};
use crate::views::{UpdateItemFailure, UpdateItemRequest, UpdateItemResponse};

/// Update an item
#[endpoint(
    tags("Items"),
    summary = "Update an item",
    parameters(
        ("item_id" = String, Path, description = "The item's unique identifier (UUID)")
    ),
    responses(
        (status_code = 200, description = "Item updated successfully", body = UpdateItemResponse),
        (status_code = 400, description = "Invalid request", body = UpdateItemFailure),
        (status_code = 404, description = "Item not found", body = UpdateItemFailure),
        (status_code = 500, description = "Internal server error", body = UpdateItemFailure),
    )
)]
pub async fn update_item(
    req: &mut Request,
    depot: &mut Depot,
    body: JsonBody<UpdateItemRequest>,
) -> Json<Result<UpdateItemResponse, UpdateItemFailure>> {
    // Parse the item_id from path
    let item_id: ItemId = match req.param::<String>("item_id") {
        Some(id_str) => match id_str.parse() {
            Ok(id) => id,
            Err(_) => {
                return Json(Err(UpdateItemFailure::InvalidParameter(
                    "Invalid item ID format".to_string(),
                )));
            }
        },
        None => {
            return Json(Err(UpdateItemFailure::InvalidParameter(
                "Missing item ID".to_string(),
            )));
        }
    };

    // Validate name if provided
    let name = match &body.name {
        Some(name_str) => match ItemName::try_new(name_str) {
            Ok(name) => Some(name),
            Err(_) => {
                return Json(Err(UpdateItemFailure::InvalidRequest(
                    "Invalid name: must be 1-255 characters".to_string(),
                )));
            }
        },
        None => None,
    };

    // Get the store from depot (injected by ItemStoreMiddleware)
    let store = match depot.obtain::<Arc<dyn ItemStore>>() {
        Ok(store) => store.clone(),
        Err(_) => {
            return Json(Err(UpdateItemFailure::Internal(
                "Store not available".to_string(),
            )));
        }
    };

    // Update the item using the service
    let service = create_item_service(store);
    match service
        .update_item(item_id, name, body.description.clone())
        .await
    {
        Ok(metadata) => Json(Ok(UpdateItemResponse::from(metadata))),
        Err(ServiceError::NotFound) => Json(Err(UpdateItemFailure::NotFound(
            "Item not found".to_string(),
        ))),
        Err(_) => Json(Err(UpdateItemFailure::Internal(
            "Failed to update item".to_string(),
        ))),
    }
}
