use std::sync::Arc;

use salvo::prelude::*;

use crate::repos::ItemStore;
use crate::services::{ItemService, create_item_service};
use crate::views::{ListItemsFailure, ListItemsResponse};

/// List all items
#[endpoint(
    tags("Items"),
    summary = "List all items",
    responses(
        (status_code = 200, description = "List of items", body = ListItemsResponse),
        (status_code = 500, description = "Internal server error", body = ListItemsFailure),
    )
)]
pub async fn list_items(depot: &mut Depot) -> Json<Result<ListItemsResponse, ListItemsFailure>> {
    // Get the store from depot (injected by ItemStoreMiddleware)
    let store = match depot.obtain::<Arc<dyn ItemStore>>() {
        Ok(store) => store.clone(),
        Err(_) => {
            return Json(Err(ListItemsFailure::Internal(
                "Store not available".to_string(),
            )));
        }
    };

    // List items using the service
    let service = create_item_service(store);
    match service.list_items().await {
        Ok(items) => Json(Ok(ListItemsResponse::new(items))),
        Err(_) => Json(Err(ListItemsFailure::Internal(
            "Failed to list items".to_string(),
        ))),
    }
}
