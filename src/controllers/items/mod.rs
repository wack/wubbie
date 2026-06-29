use salvo::prelude::*;

pub use create::create_item;
pub use delete::delete_item;
pub use list::list_items;
pub use read::get_item;
pub use update::update_item;

mod create;
mod delete;
mod list;
mod read;
mod update;

/// Items controller - provides CRUD operations for items
pub fn items_controller() -> Router {
    Router::with_path("items")
        .get(list_items)
        .post(create_item)
        .push(
            Router::with_path("{item_id}")
                .get(get_item)
                .patch(update_item)
                .delete(delete_item),
        )
}
