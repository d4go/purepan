//! 分享同步(订阅)API(进程内直调)。

use serde::Deserialize;
use serde_json::Value;

use purepan_backend::server::inproc;

use super::{ApiClient, ApiError};

pub use purepan_backend::share_sync::config::ShareSubscription;
pub use purepan_backend::share_sync::persistence::RunRecord;

#[derive(Debug, Clone, Deserialize)]
pub struct TriggerSubscriptionResponse {
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

impl ApiClient {
    pub async fn list_subscriptions(&self) -> Result<Vec<ShareSubscription>, ApiError> {
        self.request(inproc::share_sync::list_subscriptions).await
    }

    pub async fn create_subscription(&self, body: Value) -> Result<ShareSubscription, ApiError> {
        self.request(move |s| inproc::share_sync::create_subscription(s, body))
            .await
    }

    pub async fn delete_subscription(&self, id: &str) -> Result<Value, ApiError> {
        let id = id.to_string();
        self.request(move |s| inproc::share_sync::delete_subscription(s, id))
            .await
    }

    pub async fn set_subscription_enabled(&self, id: &str, enabled: bool) -> Result<Value, ApiError> {
        let id = id.to_string();
        self.request(move |s| inproc::share_sync::set_enabled(s, id, enabled))
            .await
    }

    pub async fn trigger_subscription(&self, id: &str) -> Result<TriggerSubscriptionResponse, ApiError> {
        let id = id.to_string();
        self.request(move |s| inproc::share_sync::trigger(s, id))
            .await
    }

    pub async fn resume_subscription(&self, id: &str) -> Result<Value, ApiError> {
        let id = id.to_string();
        self.request(move |s| inproc::share_sync::resume(s, id))
            .await
    }

    pub async fn list_runs(&self, id: &str, page: usize) -> Result<Vec<RunRecord>, ApiError> {
        let id = id.to_string();
        self.request(move |s| inproc::share_sync::list_runs(s, id, page))
            .await
    }
}
