use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Items::Table)
                    .if_not_exists()
                    .col(pk_auto(Items::Id))
                    .col(uuid_uniq(Items::InternalId))
                    .col(string(Items::Name))
                    .col(string_null(Items::Description))
                    .col(timestamp_with_time_zone(Items::CreatedAt))
                    .col(timestamp_with_time_zone(Items::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        // Create index on internal_id for faster lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_items_internal_id")
                    .table(Items::Table)
                    .col(Items::InternalId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Items::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Items {
    Table,
    Id,
    InternalId,
    Name,
    Description,
    CreatedAt,
    UpdatedAt,
}
