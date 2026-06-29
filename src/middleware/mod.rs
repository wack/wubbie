use salvo::prelude::*;
use sea_orm::TransactionTrait;

use crate::repos::{Transaction, TransactionError};

pub mod error_catcher;
pub mod metrics;
pub mod mocks;
pub mod otel_http;
pub mod stores;

pub use error_catcher::JsonErrorCatcher;

pub struct TransactionMiddleware;

#[async_trait]
impl Handler for TransactionMiddleware {
    async fn handle(
        &self,
        req: &mut Request,
        depot: &mut Depot,
        res: &mut Response,
        ctrl: &mut FlowCtrl,
    ) {
        let pool = crate::utils::db::pool();

        match pool.begin().await {
            Ok(db_txn) => {
                let transaction = Transaction::new(db_txn);
                depot.inject(transaction.clone());

                ctrl.call_next(req, depot, res).await;

                // Retrieve the transaction and attempt to commit or rollback
                if let Ok(txn_wrapper) = depot.obtain::<Transaction>() {
                    // Check if transaction is still available (not consumed by handlers)
                    if txn_wrapper.is_available().await {
                        // Consume the transaction to get the underlying DatabaseTransaction
                        match txn_wrapper.clone().into_inner().await {
                            Ok(db_txn) => {
                                if res
                                    .status_code
                                    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
                                    .is_success()
                                {
                                    if let Err(e) = db_txn.commit().await {
                                        tracing::error!("Failed to commit transaction: {}", e);
                                        res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
                                    }
                                } else {
                                    if let Err(e) = db_txn.rollback().await {
                                        tracing::error!("Failed to rollback transaction: {}", e);
                                    }
                                }
                            }
                            Err(TransactionError::TransactionConsumed) => {
                                tracing::warn!("Transaction was already consumed by handlers");
                                // This is actually fine - handlers may have committed/rolled back manually
                            }
                            Err(e) => {
                                tracing::error!("Failed to extract transaction: {:?}", e);
                                res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
                            }
                        }
                    } else {
                        tracing::debug!("Transaction was consumed during request processing");
                    }
                } else {
                    tracing::warn!("Transaction not found in depot");
                }
            }
            Err(e) => {
                tracing::error!("Failed to begin transaction: {}", e);
                res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }
}

pub fn transaction_middleware() -> TransactionMiddleware {
    TransactionMiddleware
}

// Services will be created on-demand from the Transaction in the depot
// Controllers will create their own service instances as needed
