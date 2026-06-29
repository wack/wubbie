pub mod items;

pub use items::{ItemService, Service as ItemServiceImpl};

use crate::repos::items::ItemStore;

/// Factory function to create an ItemService implementation with dependencies.
///
/// This function provides dependency injection for the service layer,
/// taking repository implementations and returning a concrete service.
pub fn create_item_service<R>(repo: R) -> impl ItemService
where
    R: ItemStore + 'static,
{
    ItemServiceImpl::new(repo)
}
