use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use salvo::async_trait;
use tokio::sync::RwLock;

use super::{DatabaseError, ItemStore};
use crate::domain::{ItemId, ItemMetadata, ItemName};

/// In-memory mock implementation of ItemStore for testing.
pub struct MockItemStore {
    items: Arc<RwLock<HashMap<ItemId, ItemMetadata>>>,
}

impl MockItemStore {
    /// Creates a new empty mock store
    pub fn new() -> Self {
        Self {
            items: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Creates a mock store pre-populated with items
    pub fn with_items(items: Vec<ItemMetadata>) -> Self {
        let map: HashMap<ItemId, ItemMetadata> = items.into_iter().map(|i| (i.id, i)).collect();
        Self {
            items: Arc::new(RwLock::new(map)),
        }
    }
}

impl Default for MockItemStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ItemStore for MockItemStore {
    async fn create(
        &self,
        name: ItemName,
        description: Option<String>,
    ) -> Result<ItemMetadata, DatabaseError> {
        let now = Utc::now();
        let metadata = ItemMetadata::new(ItemId::random(), name, description, now, now);

        let mut items = self.items.write().await;
        items.insert(metadata.id, metadata.clone());

        Ok(metadata)
    }

    async fn list(&self) -> Result<Vec<ItemMetadata>, DatabaseError> {
        let items = self.items.read().await;
        Ok(items.values().cloned().collect())
    }

    async fn read(&self, id: ItemId) -> Result<ItemMetadata, DatabaseError> {
        let items = self.items.read().await;
        items.get(&id).cloned().ok_or(())
    }

    async fn update(
        &self,
        id: ItemId,
        name: Option<ItemName>,
        description: Option<Option<String>>,
    ) -> Result<ItemMetadata, DatabaseError> {
        let mut items = self.items.write().await;

        let item = items.get_mut(&id).ok_or(())?;

        if let Some(new_name) = name {
            item.name = new_name;
        }

        if let Some(new_description) = description {
            item.description = new_description;
        }

        item.updated_at = Utc::now();

        Ok(item.clone())
    }

    async fn delete(&self, id: ItemId) -> Result<(), DatabaseError> {
        let mut items = self.items.write().await;
        items.remove(&id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_read() {
        let store = MockItemStore::new();
        let name = ItemName::try_new("Test Item").unwrap();

        let created = store
            .create(name.clone(), Some("A description".to_string()))
            .await
            .unwrap();

        assert_eq!(created.name.as_ref(), "Test Item");
        assert_eq!(created.description, Some("A description".to_string()));

        let read = store.read(created.id).await.unwrap();
        assert_eq!(read.id, created.id);
        assert_eq!(read.name.as_ref(), "Test Item");
    }

    #[tokio::test]
    async fn test_list() {
        let store = MockItemStore::new();

        store
            .create(ItemName::try_new("Item 1").unwrap(), None)
            .await
            .unwrap();
        store
            .create(ItemName::try_new("Item 2").unwrap(), None)
            .await
            .unwrap();

        let items = store.list().await.unwrap();
        assert_eq!(items.len(), 2);
    }

    #[tokio::test]
    async fn test_update() {
        let store = MockItemStore::new();
        let created = store
            .create(ItemName::try_new("Original").unwrap(), None)
            .await
            .unwrap();

        let updated = store
            .update(
                created.id,
                Some(ItemName::try_new("Updated").unwrap()),
                Some(Some("New description".to_string())),
            )
            .await
            .unwrap();

        assert_eq!(updated.name.as_ref(), "Updated");
        assert_eq!(updated.description, Some("New description".to_string()));
    }

    #[tokio::test]
    async fn test_delete() {
        let store = MockItemStore::new();
        let created = store
            .create(ItemName::try_new("To Delete").unwrap(), None)
            .await
            .unwrap();

        store.delete(created.id).await.unwrap();

        let result = store.read(created.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_not_found() {
        let store = MockItemStore::new();
        let result = store.read(ItemId::random()).await;
        assert!(result.is_err());
    }
}
