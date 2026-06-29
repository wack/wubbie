use std::sync::Arc;

use salvo::prelude::*;

use crate::domain::ItemId;
use crate::repos::ItemStore;
use crate::services::{ItemService, create_item_service, items::ServiceError};
use crate::views::{ReadItemFailure, ReadItemResponse};

/// Get a single item by ID
#[endpoint(
    tags("Items"),
    summary = "Get an item by ID",
    parameters(
        ("item_id" = String, Path, description = "The item's unique identifier (UUID)")
    ),
    responses(
        (status_code = 200, description = "Item details", body = ReadItemResponse),
        (status_code = 400, description = "Invalid item ID", body = ReadItemFailure),
        (status_code = 404, description = "Item not found", body = ReadItemFailure),
        (status_code = 500, description = "Internal server error", body = ReadItemFailure),
    )
)]
pub async fn get_item(
    req: &mut Request,
    depot: &mut Depot,
) -> Json<Result<ReadItemResponse, ReadItemFailure>> {
    // Parse the item_id from path
    let item_id: ItemId = match req.param::<String>("item_id") {
        Some(id_str) => match id_str.parse() {
            Ok(id) => id,
            Err(_) => {
                return Json(Err(ReadItemFailure::InvalidParameter(
                    "Invalid item ID format".to_string(),
                )));
            }
        },
        None => {
            return Json(Err(ReadItemFailure::InvalidParameter(
                "Missing item ID".to_string(),
            )));
        }
    };

    // Get the store from depot (injected by ItemStoreMiddleware)
    let store = match depot.obtain::<Arc<dyn ItemStore>>() {
        Ok(store) => store.clone(),
        Err(_) => {
            return Json(Err(ReadItemFailure::Internal(
                "Store not available".to_string(),
            )));
        }
    };

    // Get the item using the service
    let service = create_item_service(store);
    match service.get_item(item_id).await {
        Ok(metadata) => Json(Ok(ReadItemResponse::from(metadata))),
        Err(ServiceError::NotFound) => {
            Json(Err(ReadItemFailure::NotFound("Item not found".to_string())))
        }
        Err(_) => Json(Err(ReadItemFailure::Internal(
            "Failed to get item".to_string(),
        ))),
    }
}
