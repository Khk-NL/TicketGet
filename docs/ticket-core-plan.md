# Ticket Core 整合方案

## 基线与职责

主线来自 `shiyutim/tickets` 的 `master`（导入时 `388a86e`）。它已有 Tauri 1 + Rust 调度、Vue 3 工作台、SQLite 日志、统一账号、两平台活动/场次/票档解析、H5/API 下单、独立余票监控、微信通知和官方订单入口。`tasks.rs` 负责预约、互斥、取消与结果通知；`monitor.rs` 只读库存；`dm.rs` 和 `bilibili.rs` 各自负责平台接口。监控目前发现库存就结束，仅通知，不触发下单。购票成功在内部用 `succeeded` 表示，界面显示“待支付”。账号 Cookie 位于 `tickets.accounts` 的 localStorage；微信绑定凭据位于应用数据目录的 `wechat/binding.json`，均为明文。

参考 `currycan/HaTickets` 的 `master`（导入时 `4dc8e63`）。其 `mobile/` 是 Python + UIAutomator2/adbutils 的大麦 Android App 执行端：`damai_app/orchestrator.py` 编排，`page_probe.py`/`state_probe.py` 识别页面，`purchase_flow.py` 提交，`recovery_strategies.py` 处理恢复，`run_report.py` 和 `__main__.py` 输出退出码及 JSON 摘要。它不承担本项目的账号、跨平台监控、通知或 GUI。其 Desktop 路径已被上游标为不可用。Android App 的登录态保留在手机，不导出账号密码。

## 边界与数据流

`平台适配器（Damai / Bilibili）` 提供活动元数据、库存检查、购票参数校验、官方订单入口和可用执行方式。`执行器（H5/API / Android U2）` 执行一次购票并给出统一结果。`TaskManager` 只管理任务、互斥、状态转换、取消和通知，不解析大麦页面。Android 第一版只支持大麦；Bilibili 保持 API 执行器。若某活动仅 App 可购，GUI 只能提供已支持的 Android 方式，不能暗示 H5 可下单。

GUI 选账号、活动、场次、票档、观演人和执行方式 → 创建 `PurchaseTask`，或创建携带目标购票参数的 `MonitorTask` → 库存查询（只读） → 明确有票时在同一平台互斥锁下启动对应购票执行器 → 返回分类结果 → 发布任务状态 → 仅在确认创建订单时发送一次现有通知 → 展示官方订单入口并由用户人工付款。普通监控保留“发现后提醒”行为。

监控联动需要在创建时冻结购票参数，不在轮询时读取可变化的页面表单。创建时校验观演人数、配送信息、票价和执行器适用性；执行器在提交前再次校验官方当前数据。任何已发出提交请求但无法确认结果的情况进入 `needs_action`，停止自动重试，指向官方订单页，以免重复订单。Android 不把“流程走完”或 `order_submitted` 日志等同于已确认待付款；只有明确的订单/待付款证据才算 `pending_payment`。

## 状态机

`waiting → running`。普通监控：`running → found / completed / failed / cancelled`。联动监控：`running → purchasing → pending_payment / monitoring / needs_action / failed / cancelled`；可重试且未确认提交时回到 `monitoring`，库存仍无票时继续按间隔查询。购票任务：`waiting → running → pending_payment / needs_action / failed / cancelled`。实现时兼容既有 `succeeded` 快照，GUI 均显示“待付款”。风控、验证、重复/未支付订单、提交结果不明一律 `needs_action`；设备不可用为 `device_error`；已明确售罄/临时网络或限流且未提交为可重试。每个平台同一时刻最多一个执行中的购票；同一监控不得并发派生第二次购票。

## 分阶段修改与验收

1. **方案与基线**：本文件、上游远端、仓库基线；检查现有 Node/Rust 测试。推送方案提交。
2. **API 最小闭环**：`src-tauri/src/tasks.rs`、`monitor.rs`、必要时 `dm.rs`/`bilibili.rs`；`src/components/TicketWorkspace.vue`、`MonitorControls.vue`、`TaskStatus.vue`、`src/services/monitoring.js`、`runtime.js`。监控可选择“有票后自动下单”，要求完整购票参数；库存出现后执行已有平台下单逻辑，成功停止、通知、显示待付款；无票和可重试失败继续监控；不明提交结果停止。测试使用注入的库存/执行器模拟，无真实订单。保留普通监控及两平台现有购票路径。
3. **执行器分层和 Android**：新增 `src-tauri/src/executors/` 及 `android/`，隔离平台适配和进程管理；以固定上游版本引入必要的 `mobile/` Python 模块及许可证说明，或固定脚本包。Rust 负责 ADB/U2 前置检查、设备列表/独占、启动/取消、结构化日志与摘要解析。Vue 增加执行方式选择和设备页；先只放行大麦 Android，提交前校验目标场次/票档/观演人。用无设备夹具测试结果映射，再在真机上 probe 和正式流程分开验收。
4. **凭据迁移**：账号与微信令牌转为系统安全凭据存储，公开设置只保留账号 ID 和展示名称；启动时兼容读取旧格式，安全写入成功后才删除明文，失败保留原数据并明确报错。覆盖新装、迁移、失败回滚和删除。不得将 Cookie、Token 或观演人完整信息写入日志、任务快照或推送仓库。
5. **回归和上游维护**：针对 Node/Rust 测试和前端构建，检查大麦/Bilibili 原有路径；在 README 记录真机验收步骤、未验证状态及上游同步方法。`tickets-upstream` 用定期 merge/rebase 检查，`hatickets-upstream` 用固定版本对照并按适配层有选择地同步，避免直接合并两个历史。

每阶段完成并验证后单独提交并推送到 `Khk-NL/TicketGet` 的工作分支。真实活动下单、微信实际送达及 Android 真机状态必须分别记录实测结果；模拟测试只能证明本地状态转换和接口契约。
