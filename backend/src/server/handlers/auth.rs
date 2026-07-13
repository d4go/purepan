// 认证API处理器

use crate::auth::{CookieLoginAuth, CookieLoginApiRequest, QRCode, QRCodeStatus, Uid, UserAuth};
use crate::common::ProxyType;
use crate::server::{broadcast_account_list_changed, set_active_uid, AppState};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

/// 统一API响应格式
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    /// 状态码 (0: 成功, 其他: 错误码)
    pub code: i32,
    /// 消息
    pub message: String,
    /// 数据
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            code: 0,
            message: "Success".to_string(),
            data: Some(data),
        }
    }

    pub fn success_with_message(data: T, message: impl Into<String>) -> Self {
        Self {
            code: 0,
            message: message.into(),
            data: Some(data),
        }
    }

    pub fn error(code: i32, message: String) -> Self {
        Self {
            code,
            message,
            data: None,
        }
    }
}

/// 生成登录二维码
///
/// POST /api/v1/auth/qrcode/generate
pub async fn generate_qrcode(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<QRCode>>, StatusCode> {
    info!("API: 生成登录二维码");

    match state.qrcode_auth.read().await.generate_qrcode().await {
        Ok(qrcode) => {
            info!("二维码生成成功: sign={}", qrcode.sign);
            Ok(Json(ApiResponse::success(qrcode)))
        }
        Err(e) => {
            error!("二维码生成失败: {}", e);
            Ok(Json(ApiResponse::error(
                500,
                format!("Failed to generate QR code: {}", e),
            )))
        }
    }
}

/// 查询参数：sign
#[derive(Debug, Deserialize)]
pub struct QRCodeStatusQuery {
    pub sign: String,
    /// 是否处于"添加账号"模式（多账号）。
    ///
    /// 添加账号时用户通常已经有一个活跃账号，必须跳过下面"已有活跃账号即视为
    /// 登录成功"的防呆短路，否则会在用户尚未扫码时就用旧账号返回 Success，
    /// 导致前端误报"账号添加成功"。
    #[serde(default)]
    pub add: bool,
    /// 是否为首次轮询。仅首次轮询时检查“已有活跃账号”的防呆短路，
    /// 避免每次轮询都向百度发 verify_bduss 请求增加延迟。
    #[serde(default)]
    pub first: bool,
}

/// 查询扫码状态
///
/// GET /api/v1/auth/qrcode/status?sign=xxx
pub async fn qrcode_status(
    State(state): State<AppState>,
    Query(params): Query<QRCodeStatusQuery>,
) -> Result<Json<ApiResponse<QRCodeStatus>>, StatusCode> {
    info!("API: 查询扫码状态: sign={}", params.sign);

    // 防呆：检查是否已有有效的活跃账号会话
    // 从 active_uid → AccountManager 取，不再读 legacy session.json
    //
    // 注意：添加账号模式（add=true）下用户本就已有活跃账号，必须跳过此短路，
    // 否则会在尚未扫码时直接用旧账号返回 Success，误报“账号添加成功”。
    //
    // 仅在前端首次轮询（first=true）时检查，避免每次轮询都发 verify_bduss
    // 网络请求导致延迟和竞态。
    if !params.add && params.first {
        if let Some(user) = state.active_user_auth().await {
            info!(
                "检测到已有活跃账号: UID={}, 验证 BDUSS 有效性...",
                user.uid
            );

            match state.qrcode_auth.read().await.verify_bduss(&user.bduss).await {
                Ok(true) => {
                    info!("✅ BDUSS 仍然有效，直接返回登录成功状态");

                    // 确保活跃账号客户端已初始化
                    let client_initialized = state.active_client().await.is_some();
                    if !client_initialized {
                        info!("🔄 客户端未初始化，开始初始化用户资源...");
                        if let Err(e) = state.load_initial_session().await {
                            error!("❌ 初始化用户资源失败: {}", e);
                        } else {
                            info!("✅ 用户资源初始化成功");
                        }
                    }

                    // 直接返回 Success 状态，token 使用 BDUSS
                    return Ok(Json(ApiResponse::success(QRCodeStatus::Success {
                        user: user.clone(),
                        token: user.bduss.clone(),
                    })));
                }
                Ok(false) => {
                    warn!("⚠️ 持久化的 BDUSS 已失效，继续扫码流程");
                    // BDUSS 失效不再触发 session.json 清除：legacy session 已被
                    // accounts.json 取代，由 set_active_uid / force_delete 等专用路径
                    // 处理凭据失效。
                }
                Err(e) => {
                    warn!("⚠️ BDUSS 验证出错: {}，继续扫码流程", e);
                }
            }
        }
    }

    match state.qrcode_auth.read().await.poll_status(&params.sign).await {
        Ok(status) => {
            // 如果登录成功，保存会话并初始化用户资源
            //
            // ⚠️ 性能关键路径：以下只执行快速（纯本地 I/O）操作，然后立即返回
            // Success 响应。耗时的初始化（预热、管理器创建等）在后台 tokio::spawn
            // 中异步进行。
            //
            // 原因：预热需要向百度服务器发网络请求，可能耗时 5-30 秒；如果在这
            // 里同步等待，前端 axios（timeout=10s）会超时，导致前端永远拿不到
            // success 状态（百度二维码 sign 确认后可能只返回一次 v 字段，前端
            // 超时后再轮询拿到的可能是 waiting/expired，形成死循环）。
            if let QRCodeStatus::Success { ref user, .. } = status {
                info!(
                    "检测到登录成功: UID={}, 用户名={}",
                    user.uid, user.username
                );

                // 1. 保存基本会话信息（快速，纯本地文件 I/O）
                {
                    let mut session = state.session_manager.lock().await;
                    if let Err(e) = session.save_session(user).await {
                        error!("❌ 保存会话失败: {}", e);
                        // 即使保存失败也继续，不阻断登录响应
                    } else {
                        info!(
                            "✅ 会话保存成功: UID={}, BDUSS长度={}",
                            user.uid,
                            user.bduss.len()
                        );
                    }
                } // session 锁在此释放

                // 2. 设置当前用户（内存态，快速）
                *state.current_user.write().await = Some(user.clone());

                // 3. 注册账号 + 设置 active_uid + 广播事件（快速，纯本地 I/O）
                //    必须在返回响应前完成，否则前端拿到 success 后调用
                //    /auth/user 和 /accounts/list 时 active_uid 还没设置好
                if let Err(e) = add_account_and_activate(&state, user.clone()).await {
                    error!("qrcode_status add_account_and_activate 失败: {}", e);
                }

                // 4. 后台异步初始化用户资源（预热 + 管理器创建等）
                //    这些操作包含网络请求（预热），可能耗时数秒甚至更长，
                //    不能阻塞 HTTP 响应。load_initial_session 会从 AccountManager
                //    读取刚注册的活跃账号，完成客户端创建、预热、DownloadManager /
                //    UploadManager / TransferManager 初始化、ScanManager / 自动备份 /
                //    分享同步 / 离线下载监听等全部初始化。
                let state_clone = state.clone();
                let uid = user.uid;
                tokio::spawn(async move {
                    info!("🔄 后台初始化用户资源开始: UID={}", uid);
                    if let Err(e) = state_clone.load_initial_session().await {
                        error!("❌ 后台初始化用户资源失败: {}", e);
                    } else {
                        info!("✅ 后台初始化用户资源完成: UID={}", uid);
                    }
                });

                info!("✅ 扫码登录成功，已立即返回响应（初始化在后台进行）: UID={}", user.uid);
            }

            Ok(Json(ApiResponse::success(status)))
        }
        Err(e) => {
            error!("查询扫码状态失败: {}", e);
            Ok(Json(ApiResponse::error(
                500,
                format!("Failed to poll status: {}", e),
            )))
        }
    }
}

/// 获取当前用户信息
///
/// GET /api/v1/auth/user
///
/// 从 `active_uid → AccountManager.get_user` 取，符合运行时真源原则。
/// 多账号场景下返回的就是当前 active_uid 对应的账号。
pub async fn get_current_user(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    info!("🔍 API: 获取当前用户信息");

    // 从 active_uid → AccountManager 取活跃账号
    let user = match state.active_user_auth().await {
        Some(u) => u,
        None => {
            warn!("❌ 未找到活跃账号，用户未登录");
            return Ok(Json(ApiResponse::error(401, "Not logged in".to_string())));
        }
    };

    info!("✅ 找到活跃账号: UID={}, 用户名={}", user.uid, user.username);

    // 验证 BDUSS 是否仍然有效
    match state
        .qrcode_auth
        .read()
        .await
        .verify_bduss(&user.bduss)
        .await
    {
        Ok(true) => {
            // BDUSS 有效，检查活跃账号客户端是否已初始化
            info!("BDUSS 验证通过");

            let client_initialized = state.active_client().await.is_some();
            if !client_initialized {
                info!("🔄 检测到客户端未初始化，开始初始化用户资源...");
                if let Err(e) = state.load_initial_session().await {
                    error!("❌ 初始化用户资源失败: {}", e);
                    // 初始化失败不影响返回用户信息
                } else {
                    info!("✅ 用户资源初始化成功");
                }
            }

            Ok(Json(ApiResponse::success(user)))
        }
        Ok(false) => {
            // BDUSS 已失效。这里只标记 401，账号删除/切换由调用方按需触发。
            warn!("BDUSS 已失效 uid={}", user.uid);
            Ok(Json(ApiResponse::error(
                401,
                "Session expired, please login again".to_string(),
            )))
        }
        Err(e) => {
            // 验证失败（可能是网络问题），暂时允许通过
            warn!("BDUSS 验证失败: {}，暂时允许通过", e);
            Ok(Json(ApiResponse::success(user)))
        }
    }
}

/// 登出
///
/// POST /api/v1/auth/logout
///
/// 通过 `delete_account_helper(force = true)` 编排关闭活跃账号的所有
/// per-uid Manager + 移除 client_pool + 删除 AccountManager 条目 + 广播事件。
pub async fn logout(State(state): State<AppState>) -> Result<impl IntoResponse, StatusCode> {
    info!("API: 用户登出");

    // 1. 清除持久化 session（文件 + 内存缓存）— legacy 字段保留
    let clear_result = {
        let mut session = state.session_manager.lock().await;
        session.clear_session().await
    };

    // 2. 清除内存中的当前用户，确保下次登录时不携带旧状态
    *state.current_user.write().await = None;

    // 3. 清除网盘客户端（含旧 Cookie Jar），下次登录时重新创建
    *state.netdisk_client.write().await = None;

    // 🔥 多账号删除编排（force 契约）
    // - active_uid = Some(uid)：删除该 uid + force shutdown 所有 per-uid Manager
    // - active_uid = None：跳过（已无活跃账号）
    if let Some(active) = *state.active_uid.read().await {
        if let Err(e) = delete_account_helper(&state, Some(active), true).await {
            error!("delete_account_helper(active) 失败: {}", e);
        }
    } else {
        info!("登出：当前无活跃账号，跳过 force-delete 编排");
    }

    info!("已清除 current_user / netdisk_client / per-uid Managers");

    match clear_result {
        Ok(_) => {
            info!("登出成功");
            Ok(Json(ApiResponse::<()>::success(())))
        }
        Err(e) => {
            error!("登出失败: {}", e);
            Ok(Json(ApiResponse::<()>::error(
                500,
                format!("Failed to logout: {}", e),
            )))
        }
    }
}

/// Cookie 登录
///
/// POST /api/v1/auth/cookie/login
///
/// 接受从浏览器 DevTools 复制的完整 Cookie 字符串，解析出 BDUSS / PTOKEN / STOKEN
/// 等字段后验证有效性并初始化所有管理器（与二维码登录后流程完全一致）。
pub async fn cookie_login(
    State(state): State<AppState>,
    Json(req): Json<CookieLoginApiRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    info!("API: Cookie 登录");

    if req.cookies.trim().is_empty() {
        return Ok(Json(ApiResponse::<crate::auth::UserAuth>::error(
            400,
            "cookies 字段不能为空".to_string(),
        )));
    }

    // 读取当前代理配置
    let proxy_config = {
        let config_guard = state.config.read().await;
        if config_guard.network.proxy.proxy_type != ProxyType::None {
            Some(config_guard.network.proxy.clone())
        } else {
            None
        }
    };

    // 创建 Cookie 登录客户端（复用代理配置）
    let cookie_auth = match CookieLoginAuth::new_with_proxy(proxy_config.as_ref()) {
        Ok(a) => a,
        Err(e) => {
            error!("创建 Cookie 登录客户端失败: {}", e);
            return Ok(Json(ApiResponse::<crate::auth::UserAuth>::error(
                500,
                format!("创建客户端失败: {}", e),
            )));
        }
    };

    // 解析 Cookie 并验证 BDUSS
    let user = match cookie_auth.login_with_cookies(&req.cookies).await {
        Ok(u) => u,
        Err(e) => {
            error!("Cookie 登录失败: {}", e);
            return Ok(Json(ApiResponse::<crate::auth::UserAuth>::error(
                400,
                format!("{}", e),
            )));
        }
    };

    info!(
        "Cookie 验证成功: UID={}, 用户名={}，开始初始化会话...",
        user.uid, user.username
    );

    // 保存会话（先释放锁再调用 load_initial_session，避免死锁）
    {
        let mut session = state.session_manager.lock().await;
        if let Err(e) = session.save_session(&user).await {
            error!("保存会话失败: {}", e);
            return Ok(Json(ApiResponse::<crate::auth::UserAuth>::error(
                500,
                format!("保存会话失败: {}", e),
            )));
        }
        *state.current_user.write().await = Some(user.clone());
        info!("✅ 会话保存成功");
    } // session 锁在此释放

    // 记录 PTOKEN 是否存在（用于后续判断预热是否可能成功）
    let has_ptoken = user.ptoken.is_some();

    if !has_ptoken {
        warn!("Cookie 中缺少 PTOKEN，预热将被跳过 → panpsc/csrf_token/bdstoken 无法获取，转存等功能不可用");
    }

    // 🔥 登录成功后注册到 AccountManager + 切换活跃账号 + 广播事件
    if let Err(e) = add_account_and_activate(&state, user.clone()).await {
        error!("add_account_and_activate 失败: {}", e);
    }

    // 复用 load_initial_session 完成完整初始化：
    //   - 创建 NetdiskClient（含代理）
    //   - 执行预热（获取 PANPSC / csrfToken / bdstoken）
    //   - 初始化下载/上传/转存管理器
    //   - 恢复持久化任务
    //   - 启动 WebSocket / 内存监控 / 自动备份 / 离线下载监听
    if let Err(e) = state.load_initial_session().await {
        error!("初始化用户资源失败: {}", e);
        // 不阻断登录——会话已保存，用户可重试或刷新页面
    } else {
        info!("✅ Cookie 登录后初始化完成");
    }

    // 返回最新内存中的用户信息（包含预热后更新的字段）
    let final_user = state
        .current_user
        .read()
        .await
        .clone()
        .unwrap_or(user);

    // 根据预热结果决定响应 message
    let warmup_ok = final_user.panpsc.is_some() && final_user.csrf_token.is_some();
    let message = if warmup_ok {
        "登录成功，预热完成".to_string()
    } else if !has_ptoken {
        "登录成功（未预热）。文件浏览和下载可正常使用；创建文件夹、上传、转存到新目录等操作需要预热（bdstoken），可能失败。建议从浏览器 Network 请求头复制包含 PTOKEN 的完整 Cookie 以获得完整功能".to_string()
    } else {
        "登录成功（预热失败，可能为网络问题）。文件浏览和下载正常；创建文件夹、上传等操作可能受影响，可尝试重新登录".to_string()
    };

    if warmup_ok {
        info!("✅ Cookie 登录完成，预热成功: UID={}", final_user.uid);
    } else {
        warn!("⚠️ Cookie 登录完成，但预热未成功: ptoken={}, panpsc={}",
            has_ptoken, final_user.panpsc.is_some());
    }

    Ok(Json(ApiResponse::success_with_message(final_user, message)))
}

// ============================================================================
// 多账号登录 / 登出 helpers
// ============================================================================

/// 登录成功后注册账号并激活。
///
/// 调用顺序：
///   1. `account_manager.add_user(user)` — 持久化到 `accounts.json`
///   2. `budget_scheduler.add_account(uid, vip, dl_req, up_req)` — 注册到全局预算调度器
///      （未注册时分片调度会走 `acquire_chunk_permit → None`
///      回滚 + return，导致下载/上传整个停摆。这里必须按 `seed_budget_scheduler` 同款逻辑
///      把账号加入 `BudgetScheduler` 双轨；先于 `set_active_uid` 调用，确保切到 active
///      之后第一笔分片就能 acquire 到 permit。）
///   3. `set_active_uid(Some(uid))` — 写运行时真源 + 持久化镜像 + 发 `Switched`
///   4. `broadcast_account_list_changed` — 发 `ListChanged`
///   5. `broadcast_budget_recomputed` — 推 `BudgetEvent::BudgetRecomputed`
///      （`add_account` 内部已 `recompute_budget`，这里只补 WS 推送）
///
/// **幂等性**：`add_user` + `BudgetScheduler::add_account`（内部 `upsert_account`）
/// 都支持「更新已有账号」；多次调用同一 UID 安全。
///
/// 注：本 helper **不**重复执行客户端构造 / per-uid Manager 注入；这些步骤由
/// `state.load_initial_session()` 在主登录流程中按需触发。
pub async fn add_account_and_activate(state: &AppState, user: UserAuth) -> anyhow::Result<()> {
    let uid = Uid::new(user.uid);

    // 提前提取 budget 注册参数（与 `state.rs::seed_budget_scheduler` 同款逻辑）
    let vip = crate::downloader::budget_scheduler::VipType::from_raw(user.vip_type);
    let auto = user.custom_config.auto_apply_recommended;
    let dl_threads = user.custom_config.download.max_global_threads;
    let up_threads = user.custom_config.upload.max_global_threads;
    let dl_req = if auto {
        crate::downloader::budget_scheduler::RequestedSource::Auto
    } else {
        crate::downloader::budget_scheduler::RequestedSource::User(dl_threads)
    };
    let up_req = if auto {
        crate::downloader::budget_scheduler::RequestedSource::Auto
    } else {
        crate::downloader::budget_scheduler::RequestedSource::User(up_threads)
    };

    // Step 1: 写入 AccountManager（持久化到 accounts.json）
    {
        let mut mgr = state.account_manager.lock().await;
        mgr.add_user(user).await?;
    }

    // Step 2: 注册到 BudgetScheduler
    state.budget_scheduler.add_account(uid, vip, dl_req, up_req).await;
    info!(
        "add_account_and_activate: uid={} 已注册到 BudgetScheduler (vip={:?}, auto={}, dl_req={:?}, up_req={:?})",
        uid.raw(), vip, auto, dl_req, up_req
    );

    // Step 3: 切换活跃账号（唯一入口 → 持久化镜像 + 广播 Switched）
    set_active_uid(state, Some(uid)).await?;

    // Step 4: 广播账号列表变更
    broadcast_account_list_changed(state).await;

    // Step 5: 推送 BudgetRecomputed（让前端 BudgetPanel 立即看到新账号配额）
    state.broadcast_budget_recomputed().await;

    info!("add_account_and_activate: uid={} 已激活", uid.raw());
    Ok(())
}

/// 登出 / 删除账号（force 契约）。
///
/// `force = true`：调用 `force_delete_account` 编排，关闭所有 per-uid Manager。
/// `force = false`：当前实现也走 force 路径（保留兼容旧版二态语义）。
///
/// **NOTE**：当 `uid_to_delete = None` 表示「使用当前 active_uid」；
/// 若同时 `active_uid = None`，返回错误（无账号可删）。
pub async fn delete_account_helper(
    state: &AppState,
    uid_to_delete: Option<Uid>,
    _force: bool,
) -> anyhow::Result<Option<Uid>> {
    let target = match uid_to_delete {
        Some(uid) => uid,
        None => state
            .active_uid
            .read()
            .await
            .ok_or_else(|| anyhow::anyhow!("无活跃账号，无法登出"))?,
    };

    // 调用 AppState 编排
    state.force_delete_account(target).await?;

    // 读取新 active_uid（force_delete_account 内部已通过 set_active_uid 写入并广播 Switched）
    let new_active = state.account_manager.lock().await.active_uid();

    // 🔥 `force_delete_account` 已通过唯一入口 `set_active_uid`
    // 广播 `Switched` + 重绑 CloudDl WS 订阅；这里只补 `ListChanged`，避免重复广播 `Switched`。
    broadcast_account_list_changed(state).await;

    Ok(new_active)
}
