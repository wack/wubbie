use std::sync::Arc;

use salvo::prelude::*;

use crate::domain::ItemId;
use crate::repos::ItemStore;
use crate::services::{ItemService, create_item_service};
use crate::views::{DeleteItemFailure, DeleteItemResponse};

/// Delete an item
#[endpoint(
    tags("Items"),
    summary = "Delete an item",
    parameters(
        ("item_id" = String, Path, description = "The item's unique identifier (UUID)")
    ),
    responses(
        (status_code = 200, description = "Item deleted successfully", body = DeleteItemResponse),
        (status_code = 400, description = "Invalid item ID", body = DeleteItemFailure),
        (status_code = 500, description = "Internal server error", body = DeleteItemFailure),
    )
)]
pub async fn delete_item(
    req: &mut Request,
    depot: &mut Depot,
) -> Json<Result<DeleteItemResponse, DeleteItemFailure>> {
    // Parse the item_id from path
    let item_id: ItemId = match req.param::<String>("item_id") {
        Some(id_str) => match id_str.parse() {
            Ok(id) => id,
            Err(_) => {
                return Json(Err(DeleteItemFailure::InvalidParameter(
                    "Invalid item ID format".to_string(),
                )));
            }
        },
        None => {
            return Json(Err(DeleteItemFailure::InvalidParameter(
                "Missing item ID".to_string(),
            )));
        }
    };

    // Get the store from depot (injected by ItemStoreMiddleware)
    let store = match depot.obtain::<Arc<dyn ItemStore>>() {
        Ok(store) => store.clone(),
        Err(_) => {
            return Json(Err(DeleteItemFailure::Internal(
                "Store not available".to_string(),
            )));
        }
    };

    // Delete the item using the service
    let service = create_item_service(store);
    match service.delete_item(item_id).await {
        Ok(()) => Json(Ok(DeleteItemResponse::new())),
        Err(_) => Json(Err(DeleteItemFailure::Internal(
            "Failed to delete item".to_string(),
        ))),
    }
}
