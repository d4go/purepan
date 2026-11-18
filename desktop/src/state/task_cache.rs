//! 任务缓存(Global):下载/上传/转存/文件夹任务的实时状态。
//! 用 serde_json::Value 存储(与 Vue 的动态任务对象一致),WS 快照整体替换、
//! 事件按 task_id 增量更新,带时间戳守卫防止乱序覆盖。

use std::collections::HashMap;

use gpui_kit::Global;
use serde_json::Value;

#[derive(Default)]
pub struct TaskCache {
    pub downloads: HashMap<String, Value>,
    pub uploads: HashMap<String, Value>,
    pub transfers: HashMap<String, Value>,
    pub folders: HashMap<String, Value>,
    last_ts: HashMap<String, i64>,
}

impl Global for TaskCache {}

fn id_of(v: &Value) -> Option<String> {
    v.get("id").and_then(|x| x.as_str()).map(String::from)
}

fn set(v: &mut Value, key: &str, val: Value) {
    if let Some(obj) = v.as_object_mut() {
        obj.insert(key.to_string(), val);
    }
}

impl TaskCache {
    pub fn apply_snapshot(&mut self, downloads: &[Value], uploads: &[Value], transfers: &[Value], folders: &[Value]) {
        self.downloads = downloads.iter().filter_map(|v| id_of(v).map(|id| (id, v.clone()))).collect();
        self.uploads = uploads.iter().filter_map(|v| id_of(v).map(|id| (id, v.clone()))).collect();
        self.transfers = transfers.iter().filter_map(|v| id_of(v).map(|id| (id, v.clone()))).collect();
        self.folders = folders.iter().filter_map(|v| id_of(v).map(|id| (id, v.clone()))).collect();
    }

    /// 应用单条任务事件。category: download/upload/transfer/folder/backup/cloud_dl/scan。
    pub fn apply_event(&mut self, timestamp: i64, category: &str, event: &Value) {
        let etype = event.get("event_type").and_then(|x| x.as_str()).unwrap_or("");
        let task_id = event
            .get("task_id")
            .or_else(|| event.get("folder_id"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if task_id.is_empty() {
            return;
        }
        // 时间戳守卫
        if let Some(prev) = self.last_ts.get(&task_id) {
            if timestamp < *prev {
                return;
            }
        }
        self.last_ts.insert(task_id.clone(), timestamp);

        let map = match category {
            "download" => &mut self.downloads,
            "upload" => &mut self.uploads,
            "transfer" => &mut self.transfers,
            "folder" => &mut self.folders,
            _ => return,
        };

        if etype == "deleted" {
            map.remove(&task_id);
            return;
        }

        if etype == "created" {
            let mut task = event.clone();
            set(&mut task, "id", Value::String(task_id.clone()));
            if category == "folder" {
                set(&mut task, "status", Value::String("scanning".into()));
            } else {
                set(&mut task, "status", Value::String("pending".into()));
                set(&mut task, "speed", Value::from(0));
                if category == "download" {
                    set(&mut task, "downloaded_size", Value::from(0));
                } else if category == "upload" {
                    set(&mut task, "uploaded_size", Value::from(0));
                }
            }
            map.insert(task_id, task);
            return;
        }

        let Some(task) = map.get_mut(&task_id) else { return };
        match etype {
            "progress" => {
                if category == "download" {
                    set(task, "downloaded_size", event.get("downloaded_size").cloned().unwrap_or(Value::from(0)));
                    set(task, "speed", event.get("speed").cloned().unwrap_or(Value::from(0)));
                } else if category == "upload" {
                    set(task, "uploaded_size", event.get("uploaded_size").cloned().unwrap_or(Value::from(0)));
                    set(task, "speed", event.get("speed").cloned().unwrap_or(Value::from(0)));
                    set(task, "completed_chunks", event.get("completed_chunks").cloned().unwrap_or(Value::from(0)));
                    set(task, "total_chunks", event.get("total_chunks").cloned().unwrap_or(Value::from(0)));
                } else if category == "folder" {
                    set(task, "downloaded_size", event.get("downloaded_size").cloned().unwrap_or(Value::from(0)));
                    set(task, "total_size", event.get("total_size").cloned().unwrap_or(Value::from(0)));
                    set(task, "completed_count", event.get("completed_files").cloned().unwrap_or(Value::from(0)));
                    set(task, "total_files", event.get("total_files").cloned().unwrap_or(Value::from(0)));
                    set(task, "speed", event.get("speed").cloned().unwrap_or(Value::from(0)));
                    if let Some(s) = event.get("status") { set(task, "status", s.clone()); }
                } else if category == "transfer" {
                    set(task, "transferred_count", event.get("transferred_count").cloned().unwrap_or(Value::from(0)));
                    set(task, "total_count", event.get("total_count").cloned().unwrap_or(Value::from(0)));
                    if let Some(s) = event.get("status") { set(task, "status", s.clone()); }
                }
                set(task, "total_size", event.get("total_size").cloned().unwrap_or_else(|| task.get("total_size").cloned().unwrap_or(Value::from(0))));
            }
            "status_changed" => {
                if let Some(s) = event.get("new_status") { set(task, "status", s.clone()); }
                if let Some(e) = event.get("error") { if !e.is_null() { set(task, "error", e.clone()); } }
            }
            "completed" | "task_completed" => {
                set(task, "status", Value::String("completed".into()));
                if let Some(at) = event.get("completed_at") { set(task, "completed_at", at.clone()); }
                if category == "download" {
                    let total = task.get("total_size").cloned().unwrap_or(Value::from(0));
                    set(task, "downloaded_size", total);
                } else if category == "upload" {
                    let total = task.get("total_size").cloned().unwrap_or(Value::from(0));
                    set(task, "uploaded_size", total);
                    if let Some(r) = event.get("is_rapid_upload") { set(task, "is_rapid_upload", r.clone()); }
                }
                set(task, "speed", Value::from(0));
            }
            "failed" => {
                set(task, "status", Value::String("failed".into()));
                if let Some(e) = event.get("error") { set(task, "error", e.clone()); }
                set(task, "speed", Value::from(0));
            }
            "paused" => { set(task, "status", Value::String("paused".into())); set(task, "speed", Value::from(0)); }
            "resumed" => { set(task, "status", Value::String("pending".into())); }
            "cancelled" => { set(task, "status", Value::String("cancelled".into())); }
            "decrypt_progress" => {
                set(task, "status", Value::String("decrypting".into()));
                set(task, "decrypt_progress", event.get("decrypt_progress").cloned().unwrap_or(Value::from(0)));
            }
            "decrypt_completed" => {
                set(task, "status", Value::String("completed".into()));
                set(task, "decrypt_progress", Value::from(1.0));
            }
            "encrypt_progress" => {
                set(task, "status", Value::String("encrypting".into()));
                set(task, "encrypt_progress", event.get("encrypt_progress").cloned().unwrap_or(Value::from(0)));
            }
            "encrypt_completed" => {
                set(task, "encrypt_progress", Value::from(1.0));
            }
            "scan_completed" => {
                set(task, "scan_completed", Value::Bool(true));
                set(task, "total_files", event.get("total_files").cloned().unwrap_or(Value::from(0)));
                set(task, "total_size", event.get("total_size").cloned().unwrap_or(Value::from(0)));
                set(task, "status", Value::String("downloading".into()));
            }
            _ => {}
        }
    }
}

// ───────── Value 读取辅助 ─────────

pub fn v_str(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}
pub fn v_u64(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(|x| x.as_u64()).unwrap_or(0)
}
pub fn v_f64(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(|x| x.as_f64()).unwrap_or(0.0)
}
pub fn v_bool(v: &Value, key: &str) -> bool {
    v.get(key).and_then(|x| x.as_bool()).unwrap_or(false)
}