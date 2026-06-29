use salvo::async_trait;

use crate::domain::{ItemId, ItemMetadata, ItemName};
use crate::repos::items::ItemStore;

/// Service trait for Item business logic.
///
/// This trait defines the business operations for items, providing
/// a layer of abstraction between controllers and repositories.
#[async_trait]
pub trait ItemService: Send + Sync + 'static {
    /// Creates a new item with the given name and optional description.
    async fn create_item(
        &self,
        name: ItemName,
        description: Option<String>,
    ) -> Result<ItemMetadata, ServiceError>;

    /// Lists all items.
    async fn list_items(&self) -> Result<Vec<ItemMetadata>, ServiceError>;

    /// Retrieves a single item by ID.
    async fn get_item(&self, id: ItemId) -> Result<ItemMetadata, ServiceError>;

    /// Updates an item's name and/or description.
    async fn update_item(
        &self,
        id: ItemId,
        name: Option<ItemName>,
        description: Option<Option<String>>,
    ) -> Result<ItemMetadata, ServiceError>;

    /// Deletes an item by ID.
    async fn delete_item(&self, id: ItemId) -> Result<(), ServiceError>;
}

/// Error type for service operations
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("Item not found")]
    NotFound,
    #[error("Internal error")]
    Internal,
}

/// Concrete implementation of ItemService
pub struct Service<R>
where
    R: ItemStore,
{
    repo: R,
}

impl<R> Service<R>
where
    R: ItemStore,
{
    /// Creates a new Service with the given repository
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl<R> ItemService for Service<R>
where
    R: ItemStore + 'static,
{
    async fn create_item(
        &self,
        name: ItemName,
        description: Option<String>,
    ) -> Result<ItemMetadata, ServiceError> {
        self.repo
            .create(name, description)
            .await
            .map_err(|_| ServiceError::Internal)
    }

    async fn list_items(&self) -> Result<Vec<ItemMetadata>, ServiceError> {
        self.repo.list().await.map_err(|_| ServiceError::Internal)
    }

    async fn get_item(&self, id: ItemId) -> Result<ItemMetadata, ServiceError> {
        self.repo.read(id).await.map_err(|_| ServiceError::NotFound)
    }

    async fn update_item(
        &self,
        id: ItemId,
        name: Option<ItemName>,
        description: Option<Option<String>>,
    ) -> Result<ItemMetadata, ServiceError> {
        self.repo
            .update(id, name, description)
            .await
            .map_err(|_| ServiceError::NotFound)
    }

    async fn delete_item(&self, id: ItemId) -> Result<(), ServiceError> {
        self.repo
            .delete(id)
            .await
            .map_err(|_| ServiceError::Internal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::items::mocks::MockItemStore;

    #[tokio::test]
    async fn test_create_and_get_item() {
        let store = MockItemStore::new();
        let service = Service::new(store);

        let name = ItemName::try_new("Test Item").unwrap();
        let created = service
            .create_item(name, Some("Description".to_string()))
            .await
            .unwrap();

        assert_eq!(created.name.as_ref(), "Test Item");

        let fetched = service.get_item(created.id).await.unwrap();
        assert_eq!(fetched.id, created.id);
    }

    #[tokio::test]
    async fn test_get_nonexistent_item() {
        let store = MockItemStore::new();
        let service = Service::new(store);

        let result = service.get_item(ItemId::random()).await;
        assert!(matches!(result, Err(ServiceError::NotFound)));
    }

    #[tokio::test]
    async fn test_list_items() {
        let store = MockItemStore::new();
        let service = Service::new(store);

        service
            .create_item(ItemName::try_new("Item 1").unwrap(), None)
            .await
            .unwrap();
        service
            .create_item(ItemName::try_new("Item 2").unwrap(), None)
            .await
            .unwrap();

        let items = service.list_items().await.unwrap();
        assert_eq!(items.len(), 2);
    }
}
