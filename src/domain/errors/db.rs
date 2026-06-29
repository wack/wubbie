use super::{InternalError, NotFound};
use sea_orm::DbErr;

/// Certain database errors are expected, while
/// others are not. Namely, sometimes you can query for an
/// object without knowing if it exists. But most other errors
/// are unexpected, and should probably result in a 500.
#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("The requested object was not found.")]
    NotFound(NotFound),
    #[error("An unexpected error occurred when querying the database: {0}")]
    Unexpected(DbErr),
}

impl From<DbErr> for DatabaseError {
    fn from(err: DbErr) -> Self {
        match err {
            DbErr::RecordNotFound(_) => Self::NotFound(NotFound::new(err.into())),
            e => Self::Unexpected(e),
        }
    }
}

/// DatabaseErrors can be converted into InternalErrors.
/// This is useful when _any_ error returned from the database
/// is unexpected.
impl From<DatabaseError> for InternalError {
    fn from(value: DatabaseError) -> Self {
        Self::new(value.into())
    }
}
