use salvo::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Histogram for collecting HTTP status codes
#[derive(Clone)]
pub struct StatusCodeHistogram {
    /// The histogram data
    data: Arc<RwLock<HashMap<u16, u64>>>,
}

impl Default for StatusCodeHistogram {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusCodeHistogram {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Increment the count for a given status code
    pub async fn increment(&self, status_code: u16) {
        let mut data = self.data.write().await;
        *data.entry(status_code).or_insert(0) += 1;
    }

    /// Take the current histogram data and reset it (delta temporality)
    pub async fn take_snapshot(&self) -> HashMap<u16, u64> {
        let mut data = self.data.write().await;
        std::mem::take(&mut *data)
    }
}

/// Middleware for collecting HTTP status code metrics
pub struct MetricsMiddleware {
    histogram: StatusCodeHistogram,
}

impl Default for MetricsMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsMiddleware {
    pub fn new() -> Self {
        let histogram = StatusCodeHistogram::new();

        // Spawn a background task to emit metrics every minute
        let histogram_clone = histogram.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                let snapshot = histogram_clone.take_snapshot().await;

                if !snapshot.is_empty() {
                    tracing::info!("HTTP Status Code Metrics (delta): {:?}", snapshot);

                    // Emit individual metrics for each status code
                    for (status_code, count) in snapshot {
                        tracing::info!(
                            status_code = status_code,
                            count = count,
                            "HTTP status code count"
                        );
                    }
                }
            }
        });

        Self { histogram }
    }
}

#[async_trait]
impl Handler for MetricsMiddleware {
    async fn handle(
        &self,
        req: &mut Request,
        depot: &mut Depot,
        res: &mut Response,
        ctrl: &mut FlowCtrl,
    ) {
        // Continue processing the request
        ctrl.call_next(req, depot, res).await;

        // After the request is processed, collect the status code
        if let Some(status_code) = res.status_code {
            self.histogram.increment(status_code.as_u16()).await;
        }
    }
}

/// Create a new metrics middleware instance
pub fn metrics_middleware() -> MetricsMiddleware {
    MetricsMiddleware::new()
}
