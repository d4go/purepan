//! 分享同步(订阅)API(移植 frontend/src/api/shareSync.ts)。{success,data,error} 信封。

use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{ApiError, ApiClient};

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
        self.request_success(Method::GET, "/share-sync/subscriptions", None).await
    }

    pub async fn get_subscription(&self, id: &str) -> Result<ShareSubscription, ApiError> {
        self.request_success(Method::GET, &format!("/share-sync/subscriptions/{id}"), None).await
    }

    pub async fn create_subscription(&self, body: Value) -> Result<ShareSubscription, ApiError> {
        self.request_success(Method::POST, "/share-sync/subscriptions", Some(body)).await
    }

    pub async fn update_subscription(&self, id: &str, body: Value) -> Result<ShareSubscription, ApiError> {
        self.request_success(Method::PUT, &format!("/share-sync/subscriptions/{id}"), Some(body)).await
    }

    pub async fn delete_subscription(&self, id: &str) -> Result<Value, ApiError> {
        self.request_success(Method::DELETE, &format!("/share-sync/subscriptions/{id}"), None).await
    }

    pub async fn set_subscription_enabled(&self, id: &str, enabled: bool) -> Result<Value, ApiError> {
        let path = if enabled { "enable" } else { "disable" };
        self.request_success(Method::POST, &format!("/share-sync/subscriptions/{id}/{path}"), Some(json!({}))).await
    }

    pub async fn trigger_subscription(&self, id: &str) -> Result<TriggerSubscriptionResponse, ApiError> {
        self.request_success(Method::POST, &format!("/share-sync/subscriptions/{id}/trigger"), Some(json!({}))).await
    }

    pub async fn resume_subscription(&self, id: &str) -> Result<Value, ApiError> {
        self.request_success(Method::POST, &format!("/share-sync/subscriptions/{id}/resume"), Some(json!({}))).await
    }

    pub async fn list_runs(&self, id: &str, page: usize) -> Result<Vec<RunRecord>, ApiError> {
        self.request_success(Method::GET, &format!("/share-sync/subscriptions/{id}/runs?page={page}&page_size=20"), None).await
    }

    pub async fn get_run(&self, run_id: &str, page: usize) -> Result<Value, ApiError> {
        self.request_success(Method::GET, &format!("/share-sync/runs/{run_id}?page={page}&page_size=100"), None).await
    }

    pub async fn list_run_items(&self, run_id: &str, page: usize) -> Result<Value, ApiError> {
        self.request_success(Method::GET, &format!("/share-sync/runs/{run_id}/items?page={page}&page_size=100"), None).await
    }

    pub async fn list_subtasks(&self, id: &str) -> Result<Value, ApiError> {
        self.request_success(Method::GET, &format!("/share-sync/subscriptions/{id}/subtasks"), None).await
    }

    pub async fn latest_snapshot(&self, id: &str) -> Result<Value, ApiError> {
        self.request_success(Method::GET, &format!("/share-sync/subscriptions/{id}/snapshots/latest"), None).await
    }

    pub async fn preview_tree(&self, body: Value) -> Result<Value, ApiError> {
        self.request_success(Method::POST, "/share-sync/preview-tree", Some(body)).await
    }
}