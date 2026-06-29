use std::sync::Arc;

use salvo::prelude::*;

use crate::repos::{ItemStore, Transaction};

pub struct ItemStoreMiddleware;

#[async_trait]
impl Handler for ItemStoreMiddleware {
    async fn handle(
        &self,
        req: &mut Request,
        depot: &mut Depot,
        res: &mut Response,
        ctrl: &mut FlowCtrl,
    ) {
        // Get the injected Transaction from the depot
        let transaction = match depot.obtain::<Transaction>() {
            Ok(txn) => txn.clone(),
            Err(_) => {
                tracing::error!(
                    "Transaction not found in depot - ensure TransactionMiddleware runs before ItemStoreMiddleware"
                );
                res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
                return;
            }
        };

        // Create a concrete database-backed ItemStore using the transaction
        // Using Arc allows the store to be cloned cheaply by controllers
        let store: Arc<dyn ItemStore> = Arc::new(transaction);
        depot.inject(store);

        ctrl.call_next(req, depot, res).await;
    }
}

pub fn item_store_middleware() -> ItemStoreMiddleware {
    ItemStoreMiddleware
}
