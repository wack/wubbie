use chrono::Utc;
use salvo::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use super::{DatabaseError, ItemStore};
use crate::domain::{ItemId, ItemMetadata, ItemName};
use crate::models::items::{ActiveModel, Column, Entity as ItemEntity};
use crate::repos::{Transaction, TransactionError};

#[async_trait]
impl ItemStore for Transaction {
    async fn create(
        &self,
        name: ItemName,
        description: Option<String>,
    ) -> Result<ItemMetadata, DatabaseError> {
        let internal_id = Uuid::new_v4();
        let now = Utc::now();

        let model = ActiveModel {
            internal_id: Set(internal_id),
            name: Set(name.to_string()),
            description: Set(description.clone()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        self.with_transaction_async(|txn| async move {
            let _result = model.insert(&txn).await.map_err(|_| ())?;
            Ok(((), txn))
        })
        .await
        .map_err(|_: TransactionError<()>| ())?;

        Ok(ItemMetadata::new(
            ItemId::new(internal_id),
            name,
            description,
            now,
            now,
        ))
    }

    async fn list(&self) -> Result<Vec<ItemMetadata>, DatabaseError> {
        self.with_transaction_async(|txn| async move {
            let items = ItemEntity::find().all(&txn).await.map_err(|_| ())?;

            let metadata: Vec<ItemMetadata> = items
                .into_iter()
                .map(|m| {
                    ItemMetadata::new(
                        ItemId::new(m.internal_id),
                        ItemName::try_new(m.name).unwrap(),
                        m.description,
                        m.created_at.into(),
                        m.updated_at.into(),
                    )
                })
                .collect();

            Ok((metadata, txn))
        })
        .await
        .map_err(|_: TransactionError<()>| ())
    }

    async fn read(&self, id: ItemId) -> Result<ItemMetadata, DatabaseError> {
        let uuid: Uuid = *id.as_ref();

        self.with_transaction_async(|txn| async move {
            let item = ItemEntity::find()
                .filter(Column::InternalId.eq(uuid))
                .one(&txn)
                .await
                .map_err(|_| ())?
                .ok_or(())?;

            let metadata = ItemMetadata::new(
                ItemId::new(item.internal_id),
                ItemName::try_new(item.name).unwrap(),
                item.description,
                item.created_at.into(),
                item.updated_at.into(),
            );

            Ok((metadata, txn))
        })
        .await
        .map_err(|_: TransactionError<()>| ())
    }

    async fn update(
        &self,
        id: ItemId,
        name: Option<ItemName>,
        description: Option<Option<String>>,
    ) -> Result<ItemMetadata, DatabaseError> {
        let uuid: Uuid = *id.as_ref();

        self.with_transaction_async(|txn| async move {
            let item = ItemEntity::find()
                .filter(Column::InternalId.eq(uuid))
                .one(&txn)
                .await
                .map_err(|_| ())?
                .ok_or(())?;

            let mut active: ActiveModel = item.into();

            if let Some(new_name) = name {
                active.name = Set(new_name.to_string());
            }

            if let Some(new_description) = description {
                active.description = Set(new_description);
            }

            active.updated_at = Set(Utc::now().into());

            let updated = active.update(&txn).await.map_err(|_| ())?;

            let metadata = ItemMetadata::new(
                ItemId::new(updated.internal_id),
                ItemName::try_new(updated.name).unwrap(),
                updated.description,
                updated.created_at.into(),
                updated.updated_at.into(),
            );

            Ok((metadata, txn))
        })
        .await
        .map_err(|_: TransactionError<()>| ())
    }

    async fn delete(&self, id: ItemId) -> Result<(), DatabaseError> {
        let uuid: Uuid = *id.as_ref();

        self.with_transaction_async(|txn| async move {
            ItemEntity::delete_many()
                .filter(Column::InternalId.eq(uuid))
                .exec(&txn)
                .await
                .map_err(|_| ())?;

            Ok(((), txn))
        })
        .await
        .map_err(|_: TransactionError<()>| ())
    }
}
