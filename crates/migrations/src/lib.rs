pub use sea_orm_migration::prelude::*;

#[path = "0001_items.rs"]
mod m0001_items;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m0001_items::Migration)]
    }
}
