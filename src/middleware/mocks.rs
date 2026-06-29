use std::sync::Arc;

use salvo::prelude::*;

use crate::repos::{ItemStore, items::mocks::MockItemStore};

pub struct ItemStoreMockMiddleware;

#[async_trait]
impl Handler for ItemStoreMockMiddleware {
    async fn handle(
        &self,
        req: &mut Request,
        depot: &mut Depot,
        res: &mut Response,
        ctrl: &mut FlowCtrl,
    ) {
        // Inject as Arc<dyn ItemStore> to match what the items controllers obtain
        // (depot.obtain::<Arc<dyn ItemStore>>()) and the production
        // ItemStoreMiddleware. Injecting a Box here would not be found by the
        // controllers' Arc lookup, leaving routes unable to resolve the store.
        let store: Arc<dyn ItemStore> = Arc::new(MockItemStore::new());
        depot.inject(store);
        ctrl.call_next(req, depot, res).await;
    }
}

pub fn item_store_mock_middleware() -> ItemStoreMockMiddleware {
    ItemStoreMockMiddleware
}
