use crate::domain::{ItemId, ItemMetadata, ItemName};
use salvo::async_trait;

pub mod mocks;
pub mod seaorm;

/// Error type for database operations
pub type DatabaseError = ();

/// Repository trait for Item persistence operations.
///
/// This trait defines the contract for item storage, allowing for
/// different implementations (e.g., SeaORM, mocks for testing).
#[async_trait]
pub trait ItemStore: Send + Sync {
    /// Creates a new item and returns its metadata.
    async fn create(
        &self,
        name: ItemName,
        description: Option<String>,
    ) -> Result<ItemMetadata, DatabaseError>;

    /// Lists all items.
    async fn list(&self) -> Result<Vec<ItemMetadata>, DatabaseError>;

    /// Reads a single item by its ID.
    ///
    /// Returns `Ok(metadata)` if found, `Err(())` if not found.
    async fn read(&self, id: ItemId) -> Result<ItemMetadata, DatabaseError>;

    /// Updates an item's name and/or description.
    ///
    /// Returns the updated metadata.
    async fn update(
        &self,
        id: ItemId,
        name: Option<ItemName>,
        description: Option<Option<String>>,
    ) -> Result<ItemMetadata, DatabaseError>;

    /// Deletes an item by its ID.
    ///
    /// This operation is idempotent - deleting a non-existent item returns success.
    async fn delete(&self, id: ItemId) -> Result<(), DatabaseError>;
}

/// Blanket implementation for Arc-wrapped ItemStore.
/// This allows `Arc<dyn ItemStore>` to implement `ItemStore`, enabling
/// services to work with cheaply cloneable trait objects that satisfy `'static`.
#[async_trait]
impl<T: ItemStore + ?Sized> ItemStore for std::sync::Arc<T> {
    async fn create(
        &self,
        name: ItemName,
        description: Option<String>,
    ) -> Result<ItemMetadata, DatabaseError> {
        (**self).create(name, description).await
    }

    async fn list(&self) -> Result<Vec<ItemMetadata>, DatabaseError> {
        (**self).list().await
    }

    async fn read(&self, id: ItemId) -> Result<ItemMetadata, DatabaseError> {
        (**self).read(id).await
    }

    async fn update(
        &self,
        id: ItemId,
        name: Option<ItemName>,
        description: Option<Option<String>>,
    ) -> Result<ItemMetadata, DatabaseError> {
        (**self).update(id, name, description).await
    }

    async fn delete(&self, id: ItemId) -> Result<(), DatabaseError> {
        (**self).delete(id).await
    }
}

#[cfg(test)]
mod tests {
    use super::ItemStore;
    use static_assertions::assert_obj_safe;

    assert_obj_safe!(ItemStore);
}
