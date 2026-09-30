# 上游同步

主项目基线：`shiyutim/tickets` 的 `388a86e`。Android 来源：`currycan/HaTickets` 的 `4dc8e639b30033ff04a34d6820be624996cfcce5`，当前复制范围与本地补丁见 [`UPSTREAM.md`](../src-tauri/resources/hatickets/UPSTREAM.md)。本仓库的 `codex/unified-ticket-core` 保留独立提交历史，不直接合并 HaTickets 的项目结构。

新克隆的仓库先配置两个只读来源：

```bash
git remote add tickets-upstream https://github.com/shiyutim/tickets.git
git remote add hatickets-upstream https://github.com/currycan/HaTickets.git
git fetch tickets-upstream master
git fetch hatickets-upstream master
```

检查更新：

```bash
git log --oneline 388a86e..tickets-upstream/master
git log --oneline 4dc8e639..hatickets-upstream/master -- mobile shared
```

同步 tickets 时先在独立分支合并 `tickets-upstream/master`，重点检查 `tasks.rs`、`monitor.rs`、账号和微信通知接口，运行 `npm test`、`npm run build`、`cargo test --locked`，再合回工作分支。同步 HaTickets 时从目标提交重新导出 `mobile/`、`shared/`、`pyproject.toml` 和 `poetry.lock`，重放 `mobile/damai_app/__main__.py` 的结果保留补丁，更新来源版本与 Python 依赖约束，验证摘要结果映射和真机页面流程。每次同步都单独提交并记录上游提交号；上游页面定位或支付状态变化必须重新做真机验收。
