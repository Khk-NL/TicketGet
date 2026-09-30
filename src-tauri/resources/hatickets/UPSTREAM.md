# HaTickets Mobile 来源

- 来源：<https://github.com/currycan/HaTickets>
- 固定版本：`4dc8e639b30033ff04a34d6820be624996cfcce5`（`master`）
- 许可：本目录的 `LICENSE`（Apache-2.0）与 `NOTICE`
- 引入范围：`mobile/`、`shared/`、`pyproject.toml`、`poetry.lock`

`mobile/damai_app/__main__.py` 有一处本地改动：重试耗尽时保留最后的 `sold_out` / `captcha` 等结果类型，供 Ticket Core 区分继续监控与人工验证。其余上游文件保持原样。新版本同步时从 `hatickets-upstream` 的指定提交重新导出上述文件，再重放这一处适配并运行 Android 结果映射测试。

Rust 在应用数据目录中复制固定版本的 `mobile/` 与 `shared/`，启动系统 Python 子进程并读取运行摘要。Android App 登录态始终在真机，不通过此目录或配置文件传递 Cookie、Token、账号密码。执行期间的临时配置包含观演人姓名，进程结束后由 Rust 删除；请勿把应用数据目录中的运行文件提交到 Git。
