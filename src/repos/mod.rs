use sea_orm::DatabaseTransaction;
use std::sync::Arc;
use tokio::sync::Mutex;

pub use items::ItemStore;

pub mod items;

/// A thread-safe wrapper around SeaORM's DatabaseTransaction that allows
/// multiple stores to share and access the same transaction.
///
/// This struct uses Arc<Mutex<>> to provide shared ownership and mutability,
/// allowing multiple stores to obtain mutable references to the underlying
/// transaction safely across thread boundaries.
#[derive(Clone)]
pub struct Transaction {
    inner: Arc<Mutex<Option<DatabaseTransaction>>>,
}

impl Transaction {
    /// Creates a new Transaction wrapping the given DatabaseTransaction
    pub fn new(txn: DatabaseTransaction) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Some(txn))),
        }
    }

    /// Executes a closure with mutable access to the underlying transaction.
    /// Returns an error if the transaction has already been consumed.
    pub async fn with_transaction<F, R, E>(&self, f: F) -> Result<R, TransactionError<E>>
    where
        F: FnOnce(&mut DatabaseTransaction) -> Result<R, E>,
    {
        let mut guard = self.inner.lock().await;

        match guard.as_mut() {
            Some(txn) => f(txn).map_err(TransactionError::UserError),
            None => Err(TransactionError::TransactionConsumed),
        }
    }

    /// Executes an async closure with access to the underlying transaction.
    /// This version temporarily takes ownership of the transaction for the duration of the operation.
    pub async fn with_transaction_async<F, Fut, R, E>(&self, f: F) -> Result<R, TransactionError<E>>
    where
        F: FnOnce(DatabaseTransaction) -> Fut + Send,
        Fut: std::future::Future<Output = Result<(R, DatabaseTransaction), E>> + Send,
        R: Send,
        E: Send,
    {
        let mut guard = self.inner.lock().await;

        match guard.take() {
            Some(txn) => match f(txn).await {
                Ok((result, returned_txn)) => {
                    *guard = Some(returned_txn);
                    Ok(result)
                }
                Err(e) => {
                    // Transaction is lost on error
                    Err(TransactionError::UserError(e))
                }
            },
            None => Err(TransactionError::TransactionConsumed),
        }
    }

    /// Consumes the transaction, returning the underlying DatabaseTransaction.
    /// After this call, the Transaction wrapper becomes unusable.
    pub async fn into_inner(self) -> Result<DatabaseTransaction, TransactionError<()>> {
        let mut guard = self.inner.lock().await;
        guard.take().ok_or(TransactionError::TransactionConsumed)
    }

    /// Checks if the transaction is still available (not consumed)
    pub async fn is_available(&self) -> bool {
        self.inner.lock().await.is_some()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TransactionError<E> {
    #[error("Transaction lock error - mutex was poisoned")]
    LockError,
    #[error("Transaction has already been consumed")]
    TransactionConsumed,
    #[error("User operation failed: {0}")]
    UserError(E),
}

impl<E> TransactionError<E> {
    /// Maps the user error type to a different type
    pub fn map_user_error<F>(self, f: impl FnOnce(E) -> F) -> TransactionError<F> {
        match self {
            TransactionError::LockError => TransactionError::LockError,
            TransactionError::TransactionConsumed => TransactionError::TransactionConsumed,
            TransactionError::UserError(e) => TransactionError::UserError(f(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_error_mapping() {
        let error: TransactionError<String> = TransactionError::UserError("test error".to_string());
        let mapped = error.map_user_error(|s| s.len());

        match mapped {
            TransactionError::UserError(len) => assert_eq!(len, 10),
            _ => panic!("Expected UserError"),
        }

        // Test other error types
        let lock_error: TransactionError<i32> = TransactionError::LockError;
        let mapped_lock = lock_error.map_user_error(|x| x.to_string());
        assert!(matches!(mapped_lock, TransactionError::LockError));

        let consumed_error: TransactionError<i32> = TransactionError::TransactionConsumed;
        let mapped_consumed = consumed_error.map_user_error(|x| x.to_string());
        assert!(matches!(
            mapped_consumed,
            TransactionError::TransactionConsumed
        ));
    }
}
