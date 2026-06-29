Each REST API resource must have its own directory under `views/`. Top-level resources get a folder directly in `views/` (e.g. `views/users/`, `views/workspaces/`). Nested resources mirror the URL path hierarchy — for example, a resource at `workspaces/{id}/applications` lives at `views/workspaces/applications/`.

Views which declare an id (like `views/workspaces/api_keys/id.rs`) must place the `Id` declaration in a file named `id.rs`. The newtype pattern must be used for these views, using either the `view_id!` macro (preferred) or the `nutype` library.

Views which have a `display_name` string field must place the `DisplayName` type in a file named `display_name.rs` (like `views/workspaces/api_keys/display_name.rs`). The newtype pattern must be used for these views as well, wrapping the `String` type in a new type. The type name must be prefixed with the resource name (e.g., `WorkspaceDisplayName` for workspaces, `ApiKeyDisplayName` for api_keys).
