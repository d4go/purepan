# PurePan

<div align="center">

![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)
![Rust](https://img.shields.io/badge/Rust-1.87-orange.svg)
![Vue](https://img.shields.io/badge/Vue-3.5+-green.svg)
![Tauri](https://img.shields.io/badge/Tauri-2.x-purple.svg)
![Docker](https://img.shields.io/badge/Docker-ready-blue.svg)

🚀 基于 Rust 的高性能百度网盘客户端 | High-performance Baidu Netdisk client built with Rust

[功能特性](#-功能特性) • [快速开始](#-快速开始) • [技术栈](#️-技术栈) • [项目结构](#-项目结构) • [最新版本](#-最新版本) • [贡献](#-贡献)

</div>

---

## 📖 简介

PurePan 是一个使用 Rust 和 Tauri 构建的现代化百度网盘第三方客户端，提供简洁的界面和高效的上传/下载功能。

## ⭐ 支持项目发展

如果这个项目对你有帮助，请点一个 **Star** 支持！

<div align="left">

[![Star](https://img.shields.io/github/stars/d4go/purepan?style=social)](https://github.com/d4go/purepan/stargazers)

</div>

> 你的 Star 会让我更有动力持续更新 🚀

### 🎯 项目初衷

本项目主要是为了解决以下痛点：

- **高性能下载需求**：通过多线程并发下载、断点续传等技术，充分利用会员带宽，实现满速下载
- **自动备份功能**：支持将本地文件自动备份到百度网盘，实现数据的安全存储和同步
- **增强上传体验**：支持文件/文件夹上传、秒传能力与上传任务管理，让本地备份到网盘的过程更可控
- **跨平台支持**：支持 Windows、Linux、macOS 等多种平台，方便在不同设备上使用
- **现代化体验**：提供简洁美观的界面，支持实时进度显示和任务管理

### 🙏 致谢

本项目参考并受到了以下优秀项目的启发：
- [komorebiCarry/BaiduPCS-Rust](https://github.com/komorebiCarry/BaiduPCS-Rust) BaiduPCS-Go 的Rust版本，提供加密功能
- [qjfoidnh/BaiduPCS-Go](https://github.com/qjfoidnh/BaiduPCS-Go) - BaiduPCS-Go 的增强版本，提供分享链接/秒传链接转存功能
- [GangZhuo/BaiduPCS](https://github.com/GangZhuo/BaiduPCS) - 百度网盘命令行客户端

感谢这些项目的开源贡献，为本项目的开发提供了重要参考。

---

## ✨ 功能特性

### 👥 多账号与资源配额
- ✅ **多账号独立管理**：添加 / 切换 / 删除多个账号，每账号独立会话与任务队列；下载 / 上传 / 转存 / 备份 / 离线下载均按 `owner_uid` 隔离
- ✅ **实时资源配额**：机器总线程按会员权重在账号间分配，每账号展示「保底 / 上限」；自动按等级推荐（普通会员 10 / 超级会员 15）

### 🔐 认证系统
- ✅ 二维码扫码登录（百度网盘 APP 扫码）
- ✅ **Cookie 登录**：支持直接粘贴浏览器 DevTools 中复制的完整 Cookie 字符串完成登录
- ✅ 自动会话管理
- ✅ 会话持久化
- ✅ 自动登录状态验证
- ✅ 失效自动跳转登录页
- ✅ **Web 访问认证**（可选，保护 Web 界面访问）
    - 密码保护：防止未授权访问
    - TOTP 双因素认证：支持 Google Authenticator 等应用
    - 恢复码机制：丢失 TOTP 设备时的备用登录方式

### 📁 文件管理
- ✅ 浏览网盘文件和目录
- ✅ 目录导航（面包屑）
- ✅ 文件列表展示（表格视图）
- ✅ 文件信息展示（名称、大小、时间、类型）

### ⬇️ 下载引擎
- ✅ 单文件下载（多线程并发下载，8 个并发分片，可配置）
- ✅ **文件夹下载**（递归下载整个文件夹，自动扫描并创建任务）
- ✅ **批量下载**（支持多文件/文件夹同时下载）
- ✅ **下载冲突策略**（支持覆盖、跳过、自动重命名，适用于单文件/文件夹/批量下载）
- ✅ 断点续传支持
- ✅ 速度限制（可配置）
- ✅ 实时进度显示（下载速度、进度百分比、ETA）
- ✅ 任务队列管理
- ✅ 暂停/继续/删除功能
- ✅ URL 健康管理和智能淘汰策略
- ✅ **CDN 链接刷新三层检测机制**（速度异常检测、线程停滞检测、定时强制刷新）
- ✅ **下载文件资源管理器**（选择下载目录，支持最近目录记忆）

### ⬆️ 上传引擎与任务管理
- ✅ 统一的上传任务列表视图（上传管理页面）
- ✅ 支持选择本地文件或文件夹发起上传（通过本地文件资源管理器）
- ✅ **批量上传**（支持多文件/文件夹同时上传）
- ✅ **上传冲突策略**（支持智能去重、自动重命名、覆盖，可设置全局默认值或单次覆盖）
- ✅ **统一上传按钮**（文件和文件夹使用同一个上传入口）
- ✅ 上传任务进度实时展示：已上传大小、总大小、上传速度、剩余时间（ETA）
- ✅ 任务控制：暂停/继续/重试/删除
- ✅ 秒传标识：支持后端秒传时，在任务上展示"秒传"标记
- ✅ **上传最近目录记忆**（自动记录最近使用的上传目录）

### 🎨 客户端 界面
- ✅ 现代化 Vue 3 + Element Plus UI
- ✅ 响应式设计
- ✅ 实时状态更新
- ✅ 友好的用户体验

### 💻 本地文件资源管理器（上传文件选择器）
- ✅ 仿系统"资源管理器"的本地文件浏览体验
- ✅ 支持根目录、前进/后退、返回上一级、刷新等导航操作
- ✅ 支持文件/目录/文件或目录三种选择模式（根据上传场景配置）
- ✅ 支持分页加载和"加载更多"，适配大目录场景
- ✅ 提供空态/错误态 UI，操作失败可一键重试

### 🔗 转存功能
- ✅ **分享链接转存**（支持转存百度网盘分享链接到自己的网盘）
- ✅ 提取码验证（支持带提取码的分享链接）
- ✅ 转存后自动下载（可选，转存成功后自动创建下载任务）
- ✅ 转存任务管理（查看转存进度和状态）

### 🔁 分享同步（Share Sync）🆕
- ✅ **订阅分享链接**，周期/定时拉取并与上次快照对比，自动转存到网盘和/或下载到本地（仅同步变动文件）
- ✅ **多账号、双目标、可过滤**：按账号隔离，单条订阅可同时配「网盘 + 本地」，支持 include/exclude 路径
- ✅ **冲突策略**：覆盖 / 新版本 / 跳过；删除行为可控（默认保留旧文件，可选跟随分享者删除）
- ✅ **实时进度与审计**：子任务进度（速度/预计剩余）、运行历史（added/modified/removed/failed）

### ⬇️ 离线下载
- ✅ **离线下载功能**（支持通过百度网盘服务器代为下载资源到网盘空间）
- ✅ 多种链接格式支持：HTTP/HTTPS、磁力链接（magnet）、ed2k 链接
- ✅ 磁力链接自动标准化（Base32 转十六进制）
- ✅ 任务管理：添加、查询、取消、删除离线下载任务
- ✅ 实时进度显示：通过 WebSocket 实时推送任务状态和进度更新
- ✅ **自动下载功能**：任务完成后可自动下载到本地，支持配置保存路径和每次询问路径选项
- ✅ 智能轮询机制：根据任务进度预测完成时间，动态调整轮询间隔（最小 3 分钟，最大 60 分钟）
- ✅ 任务详情查看：支持查看任务的完整信息，包括文件列表、保存路径、源链接等

### 🐳 部署支持
- ✅ Docker 一键部署
- ✅ Docker Compose 支持
- ✅ 多阶段构建优化

### 📡 实时与持久化能力
- ✅ 任务持久化与断点恢复
- ✅ WebSocket 实时推送
- ✅ 日志持久化与滚动

### 🌐 网络代理
- ✅ **HTTP 和 SOCKS5 代理支持**：支持配置代理服务器转发所有网络请求
- ✅ **代理认证**：支持用户名和密码认证
- ✅ **代理测试连接**：实时检测代理可用性和延迟
- ✅ **运行状态监控**：显示代理状态（正常/已回退到直连/探测中）、抖动次数、下次探测倒计时
- ✅ **自动回退机制**：代理故障时自动回退到直连模式，可配置是否允许回退
- ✅ **即时生效**：配置保存后立即生效，无需重启应用

### 🔄 自动备份
- ✅ **上传备份**（本地 → 云端）：自动将本地文件夹备份到百度网盘
- ✅ **下载备份**（云端 → 本地）：自动将云端文件同步到本地
- ✅ **文件系统监听**：实时检测本地文件变化（Windows 使用 ReadDirectoryChangesW）
- ✅ **定时轮询兜底**：防止监听遗漏，支持间隔轮询和指定时间全量扫描
- ✅ 备份配置管理：创建/编辑/删除配置、手动触发、禁用/启用
- ✅ 备份历史记录与 SQLite 持久化

### 🔐 客户端侧加密
- ✅ **AES-256-GCM 加密算法**：端到端加密保护文件隐私
- ✅ **支持普通上传和自动备份**：灵活选择是否启用加密
- ✅ 加密密钥管理：生成、导出、删除密钥
- ✅ 加密文件使用 `.dat` 扩展名隐藏真实文件类型
- ✅ 加密快照管理：记录映射关系，支持下载自动解密
- ✅ **文件管理原始文件名显示**：加密文件在文件列表中自动还原显示原始文件名

> ⚠️ **重要提示**：请务必妥善备份以下文件，否则已加密的文件将**无法解密**！
> - `config/encryption.json`：加密密钥文件（建议同时使用界面的"导出密钥"功能备份）
> - `config/baidu-pcs.db`：数据库文件，包含加密映射表（记录加密文件与原始文件名的对应关系）

### 🔓 独立解密工具 (decrypt-cli)

为了方便用户在没有主程序的情况下解密文件，我们提供了独立的命令行解密工具 `decrypt-cli`。

**特性：**
- ✅ 独立运行，无需启动主程序
- ✅ 支持批量解密和单文件解密
- ✅ 支持 AES-256-GCM 和 ChaCha20-Poly1305 算法
- ✅ 跨平台支持（Windows、Linux、macOS）

**下载：**
- 推荐从 [`decrypt-cli-v0.1.2` 专用发布页](https://github.com/komorebiCarry/BaiduPCS-Rust/releases/tag/decrypt-cli-v0.1.2) 直接下载对应平台的 `decrypt-cli` 可执行文件
- 也可以在项目的 [Releases 页面](https://github.com/komorebiCarry/BaiduPCS-Rust/releases) 中查看全部版本

<details>
<summary>📖 <b>decrypt-cli 使用说明</b>（点击展开）</summary>

#### 准备工作

解密前需要准备以下文件：
1. `encryption.json` - 加密密钥文件（从主程序导出或备份）
2. `mapping.json` - 加密映射文件（批量解密时需要，可从主程序导出）
3. 加密的文件（`.dat` 文件）

#### 批量解密模式

```bash
# 恢复原始目录结构（使用映射文件中的原始路径）
decrypt-cli decrypt --key-file encryption.json --map mapping.json --in-dir ./encrypted --out-dir ./decrypted

# 镜像输入目录结构（按 --in-dir 的目录结构输出）
decrypt-cli decrypt --key-file encryption.json --map mapping.json --in-dir ./encrypted --out-dir ./decrypted --mirror
```

#### 单文件解密模式

```bash
# 自动尝试所有密钥
decrypt-cli decrypt --key-file encryption.json --in file.dat --out file.txt

# 指定密钥版本
decrypt-cli decrypt --key-file encryption.json --in file.dat --out file.txt --key-version 2
```

#### 参数说明

| 参数 | 说明 |
|------|------|
| `--key-file` | 密钥配置文件路径（必需） |
| `--map` | 映射文件路径（批量模式必需） |
| `--in-dir` | 输入目录（批量模式） |
| `--out-dir` | 输出目录（批量模式） |
| `--in` | 单个输入文件（单文件模式） |
| `--out` | 单个输出文件（单文件模式） |
| `--key-version` | 指定密钥版本（单文件模式可选） |
| `--mirror` | 镜像输入目录结构，而不是恢复原始路径 |

</details>

<details>
<summary>📖 <b>加密映射原理说明</b>（点击展开）</summary>

#### 文件加密
- 每个文件加密后生成唯一的 UUID 文件名（如 `a1b2c3d4-xxxx.dat`）
- 映射关系：`加密文件名 → 原始文件名` 存储在数据库中

#### 文件夹加密映射
文件夹使用**路径感知**的加密映射策略：

**加密时（原始名 → 加密名）**：根据 `(父路径, 文件夹名)` 查询或生成 UUID
```
示例：
1. 首次遇到 /backup/a/docs：
   - 查询：parent=/backup/a, name=docs → 未找到
   - 生成新 UUID：abc-123-xxx
   - 保存映射：(/backup/a, docs) → abc-123-xxx

2. 首次遇到 /backup/b/docs：
   - 查询：parent=/backup/b, name=docs → 未找到
   - 生成新 UUID：def-456-xxx（不同路径，不同 UUID）
   - 保存映射：(/backup/b, docs) → def-456-xxx

3. 再次遇到 /backup/a/docs：
   - 查询：parent=/backup/a, name=docs → 找到 abc-123-xxx
   - 复用已有加密名（保持一致性）
```

**解密时（加密名 → 原始名）**：直接根据 UUID 查询，无需父路径（UUID 全局唯一）

这种设计确保：
- ✅ 不同路径下的同名文件夹使用不同的加密名，避免冲突
- ✅ 同一路径下的文件夹多次上传时复用同一加密名，保持一致性
- ✅ 解密时通过唯一的 UUID 直接还原原始文件夹名

</details>

---

## 🚀 快速开始

<details>
<summary><b>🔐 登录流程说明</b>（点击展开）</summary>

#### 操作步骤

1. **打开百度网盘 APP**
    - 在手机上打开百度网盘 APP
    - 确保 APP 已登录你的账号

2. **扫描二维码**
    - 打开应用
    - 页面会自动显示登录二维码
    - 打开百度网盘 APP，点击"扫一扫"功能
    - 扫描网页上显示的二维码

3. **确认登录**
    - 扫描成功后，APP 会弹出确认登录提示
    - **重要：在 APP 中点击确认登录**
    - **重要：确认后，APP 不能关闭或切换到后台**，否则登录会失败

4. **等待登录完成**
    - 网页会自动轮询登录状态（通常 1-2 秒）
    - 登录成功后会自动跳转到文件管理页面

#### ⚠️ 重要提示

- **APP 必须保持打开状态**：确认登录后，不要立即在 APP 中点击确认，先稍等片刻，等待网页显示"扫码成功"后，再按提示继续操作。
- APP 不能关闭、不能退出、不能切换到后台，必须保持在前台运行，直到网页显示登录成功
- **原因说明**：网页需要通过轮询机制查询登录状态，如果 APP 关闭，百度服务器可能无法正确返回登录状态，导致登录失败
- **登录失败处理**：如果等待一段时间后仍无法登录，请：
    1. 刷新网页，重新获取二维码
    2. 重新执行上述操作步骤
    3. 确保 APP 在确认登录后保持打开状态

#### 登录状态保持

- 登录成功后，会话会自动保存到本地
- 下次启动应用时会自动恢复登录状态
- 如果会话过期，会自动跳转到登录页面重新登录

</details>

### 使用 Docker（推荐）

#### 方式一：使用预构建镜像（最简单）

**使用 docker run（推荐）**：

```bash
# 1. 创建必要的目录
mkdir -p baidupcs-rust/{config,downloads,data,logs,wal}
cd baidupcs-rust

# 2. 运行容器（使用预构建镜像）
docker run -d \
  --name PurePan \
  -p 18888:18888 \
  -v $(pwd)/config:/app/config \
  -v $(pwd)/downloads:/app/downloads \
  -v $(pwd)/data:/app/data \
  -v $(pwd)/logs:/app/logs \
  -v $(pwd)/wal:/app/wal \
  --restart unless-stopped \
  komorebicarry/baidupcs-rust:latest

# 3. 访问应用
open http://localhost:18888
```

**使用 docker-compose：**

```bash
# 1. 创建必要的目录
mkdir -p baidupcs-rust/{config,downloads,data,logs,wal}
cd baidupcs-rust

# 2. 下载 docker-compose 配置文件
# 从项目仓库下载 docker-compose.image.yml 文件

# 3. 启动服务（使用预构建镜像）
docker-compose -f docker-compose.image.yml up -d

# 4. 访问应用
open http://localhost:18888
```

#### 方式二：从源码构建

```bash
# 1. 克隆项目
git clone https://github.com/d4go/purepan.git
cd purepan

# 2. 构建并启动服务
docker-compose up -d

# 3. 访问应用
open http://localhost:18888
```

<details>
<summary><b>📋 Docker 详细说明</b>（点击展开）</summary>

**说明**：
- Docker 容器内，后端 API 服务和前端静态文件服务运行在同一个进程中
- 只需要暴露一个端口（18888）即可访问完整应用
- 前端页面和 API 调用都通过 `http://localhost:18888` 访问
- 前端在容器内部通过 `http://localhost:18888/api/v1` 调用后端 API
- **挂载目录说明**：
    - `config`：配置文件目录，包含以下重要文件：
        - `encryption.json`：加密密钥文件
        - `baidu-pcs.db`：SQLite 数据库（历史记录、加密映射表等）
        - `autobackup_configs.json`：自动备份配置
    - `downloads`：下载文件保存目录
    - `data`：会话数据目录（登录信息、会话持久化）
    - `logs`：日志文件目录（应用运行日志，支持滚动）
    - `wal`：WAL 目录（任务持久化数据，支持断点恢复）
- ⚠️ **重要提示**：如果启用了客户端侧加密功能，请务必备份以下文件：
    - `config/encryption.json`：加密密钥，**丢失后将无法解密已加密的文件**！
    - `config/baidu-pcs.db`：包含加密文件映射表，用于解密时查找原始文件名
- 预构建镜像自动发布在以下位置：
    - [GitHub Container Registry](https://github.com/komorebiCarry/BaiduPCS-Rust/pkgs/container/baidupcs-rust) - `ghcr.io/komorebicarry/baidupcs-rust:latest`（推荐，无需额外注册）
    - [Docker Hub](https://hub.docker.com/r/komorebicarry/baidupcs-rust) - `komorebicarry/baidupcs-rust:latest`（可选，需要配置 secrets）
- 当创建 Git 标签（如 `v1.0.0`）时，GitHub Actions 会自动构建并推送 Docker 镜像
- **Linux 用户推荐使用 Docker Hub 镜像**（更通用，无需登录）：`komorebicarry/baidupcs-rust:latest`
- **如果镜像拉取较慢**，也可以从 [GitHub Releases](https://github.com/komorebiCarry/BaiduPCS-Rust/releases) 页面直接下载 Docker 镜像文件（`BaiduPCS-Rust-{版本号}-docker.tar.gz`），然后使用以下命令加载：
  ```bash
  # 下载镜像文件后，加载镜像
  docker load < BaiduPCS-Rust-v1.7.0-docker.tar.gz
  
  # 加载后可以使用镜像运行容器
  docker run -d \
    --name PurePan \
    -p 18888:18888 \
    -v $(pwd)/config:/app/config \
    -v $(pwd)/downloads:/app/downloads \
    -v $(pwd)/data:/app/data \
    -v $(pwd)/logs:/app/logs \
    -v $(pwd)/wal:/app/wal \
    --restart unless-stopped \
    baidupcs-rust:v1.7.0
  ```

**docker-compose 文件说明**：

本项目提供三个 Docker Compose 配置文件，适用于不同场景：

| 文件 | 用途 | 适用场景 |
|------|------|----------|
| `docker-compose.yml` | 从源码构建 | 本地源码构建部署 |
| `docker-compose.dev.yml` | 开发环境 | 前后端分离热重载开发 |
| `docker-compose.image.yml` | 预构建镜像 | 生产环境直接使用官方镜像 |

**docker-compose.dev.yml（开发环境）**：
- 后端服务使用 `cargo watch` 实现热重载，代码变更自动重新编译
- 前端服务使用 Vite 开发服务器，支持 HMR 热更新
- 后端端口 `18888`，前端端口 `5173`
- 需要 `backend/Dockerfile.dev` 和 `frontend/Dockerfile.dev`

```bash
# 启动开发环境
docker-compose -f docker-compose.dev.yml up -d
```

**docker-compose.image.yml（预构建镜像）**：
- 直接使用官方预构建镜像，无需本地编译
- 适合快速部署到生产环境
- 支持健康检查、资源限制和自动重启

```bash
# 使用预构建镜像
docker-compose -f docker-compose.image.yml up -d
```

</details>

### 手动安装

#### 前提条件

- Rust 1.87（项目通过 `rust-toolchain.toml` 固定为 1.87.0，本地 / Docker / CI 保持一致）
- Node.js 18+
- npm 或 pnpm

#### 后端

```bash
cd backend
cargo build --release
cargo run --release
```

#### 前端

```bash
cd frontend
npm install
npm run build
# 或者开发模式
npm run dev
```

访问 http://localhost:5173（开发模式）或 http://localhost:18888（生产模式）

---

## 🖥️ Tauri 桌面端构建

项目支持使用 [Tauri 2.x](https://tauri.app/) 构建跨平台桌面应用。

### 前提条件

- 安装 Rust 工具链（1.87+）
- 安装 Node.js 18+ 和 npm
- 安装系统依赖（根据平台）：
  - **Windows**：Microsoft Visual C++ Build Tools
  - **Linux**：`libgtk-3-dev`, `libwebkit2gtk-4.0-dev` 等
  - **macOS**：Xcode Command Line Tools

### 安装 Tauri CLI

```bash
cargo install tauri-cli
```

### 安装前端依赖

```bash
cd frontend
npm install
```

### 开发模式

```bash
# 在项目根目录执行
cargo tauri dev
```

这会同时启动前端开发服务器和 Tauri 桌面窗口，支持热重载。

### 生产构建

```bash
# 在项目根目录执行
cargo tauri build
```

构建完成后，安装包位于 `src-tauri/target/release/bundle/` 目录下：
- **Windows**：`.msi` 和 `.exe` 安装程序
- **Linux**：`.deb`、`.rpm` 和 `.AppImage`
- **macOS**：`.dmg` 和 `.app`

### 项目配置

Tauri 配置文件位于 `src-tauri/tauri.conf.json`，主要配置项：
- 应用窗口尺寸、标题等外观属性
- 前端构建产物路径（`frontendDist`）
- 开发服务器 URL
- 打包图标和分发目标

---

## 📝 平台支持说明

### 测试平台

本项目主要在以下平台进行测试：

- ✅ **Windows** - 主要开发和测试平台
- ✅ **Docker** - 容器化部署，支持跨平台运行

### 其他平台

虽然项目理论上支持 Linux、macOS 等其他平台，但**目前不会进行主动测试**。如果您在其他平台使用遇到问题，欢迎提交 Issue。

### 多平台打包

项目使用 **GitHub Actions** 进行自动化多平台构建和打包，支持：

- Windows (x86_64)
- Linux (x86_64, ARM64)
- macOS (x86_64, ARM64)

预编译的二进制文件可在 [Releases](https://github.com/komorebiCarry/BaiduPCS-Rust/releases) 页面下载。

---

## 🛠️ 技术栈

### 后端

- **语言**: Rust 1.87
- **框架**: Axum 0.7（Web 框架）
- **异步运行时**: Tokio
- **HTTP 客户端**: Reqwest
- **序列化**: Serde + Serde JSON
- **日志**: Tracing

### 前端

- **框架**: Vue 3.5+（Composition API）
- **语言**: TypeScript 5.6+
- **构建工具**: Vite 6
- **UI 库**: Element Plus 2.9+
- **状态管理**: Pinia
- **路由**: Vue Router 4
- **HTTP 客户端**: Axios

### 桌面端

- **框架**: Tauri 2.x
- **构建工具**: Cargo + Tauri CLI

### 部署

- **容器化**: Docker + Docker Compose
- **多阶段构建**: 优化镜像大小
- **健康检查**: 自动故障检测

---

<details>
<summary><b>📁 项目结构</b>（点击展开）</summary>

```
BaiduPCS-Rust/
├── backend/                # Rust 后端
│   ├── src/
│   │   ├── auth/          # 认证模块
│   │   ├── netdisk/       # 网盘 API
│   │   ├── downloader/    # 下载引擎
│   │   ├── uploader/      # 上传引擎
│   │   ├── transfer/      # 转存模块（分享链接转存）
│   │   ├── common/        # 公共模块（CDN刷新检测等）
│   │   ├── filesystem/    # 文件系统访问模块
│   │   ├── config/        # 配置管理
│   │   ├── server/        # Web 服务器
│   │   └── sign/          # 签名算法
│   └── Cargo.toml
├── frontend/               # Vue 3 前端
│   ├── src/
│   │   ├── views/         # 页面组件
│   │   ├── components/    # 公共组件
│   │   ├── api/           # API 封装
│   │   ├── stores/        # Pinia 状态
│   │   └── router/        # 路由配置
│   └── package.json
├── src-tauri/              # Tauri 桌面端
│   ├── src/
│   │   ├── ipc.rs         # IPC 通信层
│   │   └── main.rs        # 桌面端入口
│   ├── Cargo.toml
│   └── tauri.conf.json    # Tauri 配置
├── decrypt-cli/            # 独立解密 CLI 工具
│   ├── src/
│   │   ├── cli.rs         # CLI 参数解析
│   │   ├── decrypt_engine.rs # 解密引擎
│   │   ├── file_parser.rs    # 文件解析
│   │   └── key_loader.rs  # 密钥加载
│   └── Cargo.toml
├── config/                 # 配置文件目录
│   ├── app.toml           # 应用主配置
│   ├── app.toml.example   # 配置模板
│   └── ...                # 加密密钥、数据库等运行时文件
├── scripts/                # 构建脚本
│   ├── build.sh
│   ├── deploy.sh
│   ├── dev.sh
│   └── test.sh
├── tests/                  # 测试脚本
│   ├── integration_test.sh
│   └── performance_test.sh
├── Dockerfile              # Docker 镜像
├── docker-compose.yml      # 生产环境（源码构建）
├── docker-compose.dev.yml  # 开发环境
├── docker-compose.image.yml # 预构建镜像
├── rustfmt.toml            # Rust 代码格式配置
├── .gitignore              # Git 忽略文件
├── LICENSE                 # Apache License 2.0
└── README.md               # 项目说明
```

</details>

---

## ⚙️ 配置

配置文件位于 `config/app.toml`:

```toml
[server]
host = "127.0.0.1"
port = 8080
cors_origins = ["*"]

[download]
download_dir = "downloads"
max_global_threads = 10
max_concurrent_tasks = 2
chunk_size_mb = 10
max_retries = 3
```

<details>
<summary><b>📋 配置参数说明</b>（点击展开）</summary>

#### 下载配置参数

- **`max_global_threads`**: 全局最大线程数（所有下载任务共享的并发分片数）
- **`max_concurrent_tasks`**: 最大同时下载文件数
- **`chunk_size_mb`**: 每个分片的大小（单位: MB）
- **`max_retries`**: 下载失败后的最大重试次数

#### 普通用户配置建议

普通用户请将**全局最大线程数**（`max_global_threads`）和**最大同时下载数**（`max_concurrent_tasks`）都设置为 1。

```toml
[download]
max_global_threads = 1
max_concurrent_tasks = 1
```

**⚠️ 注意**：调大线程数只会在短时间内提升下载速度，且极易很快触发限速，导致几小时至几天内账号在各客户端都接近 0 速。

#### SVIP 用户配置建议

SVIP 用户建议**全局最大线程数**（`max_global_threads`）设置为 10 以上，根据实际带宽可调大，但不建议超过 20。可以配合**最大同时下载数**（`max_concurrent_tasks`）调整，**注意最大同时下载数越大不代表速度越快**。

```toml
[download]
max_global_threads = 10    # 建议 10-20
max_concurrent_tasks = 2    # 可根据需求调整
```

</details>

---

## 🔐 Web 认证配置

Web 认证用于保护部署在公网环境或共享网络中的 Web 管理界面，防止未授权访问。

### 配置文件

在 `config/app.toml` 中配置 `[web_auth]` 段：

```toml
[web_auth]
# 是否启用 Web 访问认证
enabled = false

# 认证模式："none" | "password" | "totp" | "password_totp"
mode = "none"
```

### 认证模式说明

| 模式 | 说明 |
|------|------|
| `none` | 无认证（默认） |
| `password` | 仅密码认证 |
| `totp` | 仅 TOTP 双因素认证 |
| `password_totp` | 密码 + TOTP 双因素认证 |

### 启用步骤

1. 在系统设置 → Web 访问认证页面中，先设置认证密码
2. 选择认证模式（如 `password_totp`）
3. 保存设置后，系统自动启用认证

### TOTP 双因素认证

- 启用 `totp` 或 `password_totp` 模式后，系统会生成 TOTP 密钥二维码
- 使用 Google Authenticator、Microsoft Authenticator 等应用扫描二维码
- 每次登录时，除了密码还需输入六位动态验证码

### 恢复码

- 启用 TOTP 后，系统会生成一组**恢复码**
- 当 TOTP 设备丢失或无法使用时，可使用恢复码登录
- 每个恢复码只能使用一次
- 建议将恢复码妥善保存到安全位置

### 紧急恢复

如果因配置错误导致无法登录，可直接编辑 `config/app.toml`，将 `enabled` 设为 `false` 临时禁用认证：

```toml
[web_auth]
enabled = false
mode = "none"
```

重启应用后即可无认证访问，然后在界面上重新配置。

---

## 🧪 测试

```bash
# 运行所有测试
./scripts/test.sh

# 后端测试
cd backend
cargo test

# 集成测试
./tests/integration_test.sh

# 性能测试
./tests/performance_test.sh
```

---

## 📈 性能指标

| 指标 | 数值 |
|------|------|
| Docker 镜像大小 | ~150MB |
| 启动时间 | 5-10 秒 |
| 内存占用（空闲） | 100-200MB |
| 内存占用（下载） | 200-500MB |
| 并发下载分片 | 最多 16 个 |

---

## 🗺️ 路线图

- [x] ✅ 基础架构
- [x] ✅ 认证模块（二维码登录）
- [x] ✅ 文件浏览
- [x] ✅ 下载引擎（多线程 + 断点续传）
- [x] ✅ Web 服务器（RESTful API）
- [x] ✅ 前端界面（Vue 3）
- [x] ✅ Docker 部署
- [x] ✅ 文件夹下载（目录下载）
- [x] ✅ 批量下载（多文件选择下载）
- [x] ✅ 批量上传（多文件/文件夹上传）
- [x] ✅ 上传功能
- [x] ✅ 百度网盘新建文件夹功能（⚠️ 可能需要重新登入才可使用）
- [x] ✅ CDN链接刷新三层检测机制（速度异常检测、线程停滞检测、定时强制刷新）
- [x] ✅ 转存分享链接功能
- [x] ✅ 上传下载最近目录记忆
- [x] ✅ 下载文件资源管理器
- [x] ✅ 任务持久化
- [x] ✅ WebSocket 实时推送与心跳/重连
- [x] ✅ 日志持久化与滚动
- [x] ✅ 移动端适配
- [x] ✅ 自动备份
- [x] ✅ 客户端侧加密（支持普通上传和自动备份）
- [x] ✅ Web 访问认证（密码保护 + TOTP 双因素认证）
- [x] ✅ 离线下载功能（支持 HTTP/HTTPS/磁力链接/ed2k，自动下载，智能轮询）
- [x] ✅ 网络代理支持（HTTP/SOCKS5 代理，支持认证、测试连接、运行状态监控、自动回退）
- [x] ✅ 批量任务操作（下载/上传任务批量暂停/继续/清除）
- [x] ✅ Tauri 桌面端支持
- [ ] 📝 下载/上传 SSD 缓冲区（针对 NAS 用户：① 减少 HDD 活跃时间，让硬盘更多休眠；② 提升传输效率，仅网速 > 磁盘速度时有效）

---

## 🤝 贡献

欢迎贡献代码！请遵循以下步骤：

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改（提交信息遵循 [Conventional Commits](https://www.conventionalcommits.org/)，例如 `git commit -m 'feat: add some amazing feature'`）
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 开启 Pull Request

### 开发规范

- **Rust**: 遵循 Rust 官方代码风格（使用 `cargo fmt` 和 `cargo clippy`）
- **TypeScript**: 遵循 Vue 3 最佳实践
- **提交信息**: 遵循 [Conventional Commits](https://www.conventionalcommits.org/) 规范（`feat`/`fix`/`docs`/`chore` 等）；PR 标题与每个 commit 都会被 CI 校验

---

## 📄 许可证

本项目采用 Apache License 2.0 许可证 - 详见 [LICENSE](LICENSE) 文件

---

## ⚠️ 免责声明

本项目仅供学习和研究使用，请勿用于商业用途。使用本工具产生的任何问题与本项目无关。

---

## 🙏 致谢
- [komorebiCarry/BaiduPCS-Rust](https://github.com/komorebiCarry/BaiduPCS-Rust) - 本项目的重要参考
- [qjfoidnh/BaiduPCS-Go](https://github.com/qjfoidnh/BaiduPCS-Go) - 本项目的重要参考
- [GangZhuo/BaiduPCS](https://github.com/GangZhuo/BaiduPCS) - 原始项目灵感来源
- [Rust 社区](https://www.rust-lang.org/)
- [Vue.js 社区](https://vuejs.org/)
- [Element Plus](https://element-plus.org/)
- [tauri 社区](https://v2.tauri.org.cn/)
---

<div align="center">

**⭐ 如果这个项目对你有帮助，请给一个 Star！**

Made with ❤️ by Rust + Tauri + Vue 3

</div>
