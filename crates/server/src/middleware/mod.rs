use std::time::Duration;

use tower::limit::RateLimitLayer;
use tower::ServiceBuilder;

/// Build `RateLimitLayer` if rate limiting is configured.
pub fn rate_limit_layer(rps: u64) -> RateLimitLayer {
    RateLimitLayer::new(rps, Duration::from_secs(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_layer_creates() {
        let _layer = rate_limit_layer(100);
    }
}
