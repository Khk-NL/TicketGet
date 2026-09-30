use crate::tasks::{order_list_url, Outcome, TaskContext};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, path::{Path, PathBuf}, process::Stdio};
use tauri::{AppHandle, Manager};
use tokio::{io::{AsyncBufReadExt, AsyncRead, BufReader}, process::Command, sync::mpsc};
use uuid::Uuid;

const SOURCE_VERSION: &str = "4dc8e63";
const DEPENDENCIES: &[&str] = &["uiautomator2>=3.2,<4", "adbutils>=2.9,<3", "selenium>=4.22,<4.28"];

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AndroidConfig {
    serial: String,
    keyword: String,
    users: Vec<String>,
    city: String,
    date: String,
    price: String,
    #[serde(default)]
    price_index: u32,
    #[serde(default)]
    target_title: String,
    #[serde(default)]
    target_venue: String,
    #[serde(default)]
    python_path: String,
    #[serde(default)]
    adb_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidDevice {
    serial: String,
    state: String,
    detail: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidEnvironment {
    python: String,
    python_version: String,
    python_supported: bool,
    uiautomator2_ready: bool,
    adb: String,
    adb_ready: bool,
    devices: Vec<AndroidDevice>,
}

#[derive(Deserialize)]
struct RunSummary {
    outcome: String,
    exit_code: i32,
    mode: Option<String>,
    terminal_reason: Option<String>,
}

struct RunFiles { config: PathBuf, result: PathBuf }
impl Drop for RunFiles {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.config);
        let _ = fs::remove_file(&self.result);
    }
}

pub fn validate(value: &Value) -> Result<(), String> {
    let config: AndroidConfig = serde_json::from_value(value["android"].clone())
        .map_err(|_| "Android 购票参数不完整")?;
    let count = value["count"].as_u64().ok_or("购买数量无效")?;
    if count == 0 || count > 20 || config.users.len() != count as usize {
        return Err("Android 观演人姓名数量须与购买张数一致".into());
    }
    for field in [&config.serial, &config.keyword, &config.city, &config.date, &config.price] {
        if field.trim().is_empty() || field.len() > 200 || field.chars().any(char::is_control) {
            return Err("请填写有效的设备、活动、城市、场次和 App 票档原文".into());
        }
    }
    if config.users.iter().any(|name| name.trim().is_empty() || name.len() > 100 || name.chars().any(char::is_control))
        || config.users.iter().collect::<std::collections::HashSet<_>>().len() != config.users.len() {
        return Err("Android 观演人姓名不能为空或重复".into());
    }
    if config.target_title.len() > 200 || config.target_venue.len() > 200 || config.price_index > 100 {
        return Err("Android 活动匹配参数无效".into());
    }
    Ok(())
}

fn app_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path_resolver().app_data_dir().ok_or("无法找到应用数据目录".into())
}

fn managed_python(app: &AppHandle) -> Result<PathBuf, String> {
    let root = app_dir(app)?.join("android").join("venv");
    Ok(if cfg!(windows) { root.join("Scripts").join("python.exe") } else { root.join("bin").join("python") })
}

fn python(app: &AppHandle, preferred: &str) -> Result<PathBuf, String> {
    let managed = managed_python(app)?;
    Ok(if managed.is_file() { managed } else if preferred.trim().is_empty() { PathBuf::from("python") } else { PathBuf::from(preferred.trim()) })
}

fn adb(preferred: &str) -> PathBuf {
    if preferred.trim().is_empty() { PathBuf::from("adb") } else { PathBuf::from(preferred.trim()) }
}

#[tauri::command]
pub async fn android_connect_local_device(adb_path: String, address: String) -> Result<String, String> {
    let (host, port) = address.trim().split_once(':').ok_or("请输入本机地址和端口，例如 127.0.0.1:7555")?;
    if !matches!(host, "127.0.0.1" | "localhost") || port.parse::<u16>().ok().filter(|value| *value > 0).is_none() {
        return Err("仅支持连接本机模拟器地址，例如 127.0.0.1:7555".into());
    }
    output(&adb(&adb_path), &["connect", address.trim()]).await
}

async fn output(program: &Path, args: &[&str]) -> Result<String, String> {
    let result = Command::new(program).args(args).output().await.map_err(|e| format!("无法启动 {}：{e}", program.display()))?;
    if !result.status.success() { return Err(String::from_utf8_lossy(&result.stderr).chars().take(300).collect()); }
    Ok(String::from_utf8_lossy(&result.stdout).trim().to_string())
}

fn supported_python(text: &str) -> bool {
    let mut parts = text.trim().strip_prefix("Python ").unwrap_or("").split('.');
    matches!((parts.next(), parts.next()), (Some("3"), Some("10" | "11" | "12" | "13")))
}

fn parse_devices(text: &str) -> Vec<AndroidDevice> {
    text.lines().filter_map(|line| {
        let mut fields = line.split_whitespace();
        let serial = fields.next()?;
        let state = fields.next()?;
        if serial == "List" || serial.starts_with('*') { return None; }
        Some(AndroidDevice { serial: serial.into(), state: state.into(), detail: fields.collect::<Vec<_>>().join(" ") })
    }).collect()
}

#[tauri::command]
pub async fn android_environment(app: AppHandle, python_path: String, adb_path: String) -> Result<AndroidEnvironment, String> {
    let py = python(&app, &python_path)?;
    let adb = adb(&adb_path);
    let version = output(&py, &["--version"]).await.unwrap_or_default();
    let supported = supported_python(&version);
    let uiautomator2_ready = supported && output(&py, &["-c", "import uiautomator2, adbutils, selenium; print('ready')"]).await.is_ok();
    let devices_output = output(&adb, &["devices", "-l"]).await;
    Ok(AndroidEnvironment { python: py.display().to_string(), python_version: version,
        python_supported: supported, uiautomator2_ready,
        adb: adb.display().to_string(), adb_ready: devices_output.is_ok(),
        devices: devices_output.map(|text| parse_devices(&text)).unwrap_or_default() })
}

#[tauri::command]
pub async fn setup_android_environment(app: AppHandle, python_path: String) -> Result<String, String> {
    let base = if python_path.trim().is_empty() { PathBuf::from("python") } else { PathBuf::from(python_path.trim()) };
    let version = output(&base, &["--version"]).await?;
    if !supported_python(&version) { return Err("HaTickets 需要 Python 3.10–3.13，请选择受支持的 Python 路径".into()); }
    let root = app_dir(&app)?.join("android").join("venv");
    if !managed_python(&app)?.is_file() {
        fs::create_dir_all(root.parent().ok_or("环境目录无效")?).map_err(|e| e.to_string())?;
        output(&base, &["-m", "venv", root.to_str().ok_or("环境目录无效")?]).await?;
    }
    let managed = managed_python(&app)?;
    let mut args = vec!["-m", "pip", "install", "--disable-pip-version-check"];
    args.extend_from_slice(DEPENDENCIES);
    output(&managed, &args).await?;
    Ok(format!("UIAutomator2 环境已安装：{}", managed.display()))
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(from).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let target = to.join(entry.file_name());
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn source_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let target = app_dir(app)?.join("android").join(format!("hatickets-{SOURCE_VERSION}"));
    if target.join(".ready").is_file() { return Ok(target); }
    let resource = app.path_resolver().resolve_resource("resources/hatickets/mobile/__init__.py")
        .ok_or("安装包缺少 HaTickets 执行资源")?;
    let source = resource.parent().and_then(Path::parent).ok_or("HaTickets 资源路径无效")?;
    copy_tree(&source.join("mobile"), &target.join("mobile"))?;
    copy_tree(&source.join("shared"), &target.join("shared"))?;
    fs::write(target.join(".ready"), SOURCE_VERSION).map_err(|e| e.to_string())?;
    Ok(target)
}

fn summary_outcome(summary: &RunSummary, order_url: &str) -> Outcome {
    if summary.exit_code == 0 && summary.mode.as_deref() == Some("submit")
        && matches!(summary.outcome.as_str(), "order_submitted" | "order_pending_payment") {
        let message = if summary.outcome == "order_pending_payment" {
            "大麦 App 检测到待付款订单，请核对是否为目标活动并人工付款"
        } else {
            "大麦 App 已确认提交后的付款页面，请人工完成付款"
        };
        return Outcome { status: "succeeded", message: message.into(), order_url: Some(order_url.into()) };
    }
    if summary.exit_code == 12 { return Outcome { status: "device_error", message: "Android 设备或运行环境出错，请检查设备连接与执行日志".into(), order_url: None }; }
    if summary.outcome == "captcha" { return Outcome { status: "needs_action", message: "大麦 App 需要人工完成验证，请在手机处理后重新启动任务".into(), order_url: Some(order_url.into()) }; }
    if summary.exit_code == 10 || summary.terminal_reason.as_deref() == Some("sold_out") {
        return Outcome { status: "failed", message: "Android 本轮未创建订单，可在下一次库存检测时重试".into(), order_url: None };
    }
    Outcome { status: "needs_action", message: "Android 订单状态需要人工核对，请先检查大麦 App 待付款订单，避免重复下单".into(), order_url: Some(order_url.into()) }
}

async fn read_pipe<R: AsyncRead + Unpin>(pipe: R, sender: mpsc::Sender<String>) {
    let mut lines = BufReader::new(pipe).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if sender.send(line.chars().take(400).collect()).await.is_err() { break; }
    }
}

pub async fn run(context: &TaskContext) -> Result<Outcome, String> {
    validate(&context.request.config)?;
    let config: AndroidConfig = serde_json::from_value(context.request.config["android"].clone()).map_err(|_| "Android 参数无效")?;
    let environment = android_environment(context.app.clone(), config.python_path.clone(), config.adb_path.clone()).await?;
    if !environment.python_supported || !environment.uiautomator2_ready || !environment.adb_ready {
        return Ok(Outcome { status: "device_error", message: "请在 Android 设备页完成 Python 3.10–3.13、UIAutomator2 和 ADB 检查".into(), order_url: None });
    }
    if !environment.devices.iter().any(|device| device.serial == config.serial && device.state == "device") {
        return Ok(Outcome { status: "device_error", message: "所选 Android 设备未连接或尚未授权 USB 调试".into(), order_url: None });
    }
    let root = source_dir(&context.app)?;
    let run_dir = app_dir(&context.app)?.join("android").join("runs");
    fs::create_dir_all(&run_dir).map_err(|e| e.to_string())?;
    let id = Uuid::new_v4().to_string();
    let files = RunFiles { config: run_dir.join(format!("{id}.json")), result: run_dir.join(format!("{id}.result.json")) };
    let payload = json!({
        "serial": config.serial, "keyword": config.keyword, "users": config.users,
        "city": config.city, "date": config.date, "price": config.price,
        "price_index": config.price_index,
        "target_title": if config.target_title.is_empty() { Value::Null } else { json!(config.target_title) },
        "target_venue": if config.target_venue.is_empty() { Value::Null } else { json!(config.target_venue) },
        "probe_only": false,
        "if_commit_order": true, "auto_navigate": true, "sell_start_time": null,
        "rush_mode": false, "fast_retry_count": 2, "fast_retry_interval_ms": 1000
    });
    fs::write(&files.config, payload.to_string()).map_err(|e| e.to_string())?;
    context.report("running", "已连接 Android 设备，正在执行大麦 App 购票流程", 1, None);
    let mut command = Command::new(&environment.python);
    command.args(["-m", "mobile.damai_app", "--serial", &config.serial, "--result-json"])
        .arg(&files.result).current_dir(root).env("HATICKETS_CONFIG_PATH", &files.config)
        .env("PYTHONDONTWRITEBYTECODE", "1").kill_on_drop(true)
        .stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)] { use std::os::windows::process::CommandExt; command.as_std_mut().creation_flags(0x08000000); }
    let mut child = command.spawn().map_err(|e| format!("无法启动 Android 执行器：{e}"))?;
    let (sender, mut logs) = mpsc::channel::<String>(64);
    if let Some(pipe) = child.stdout.take() { tokio::spawn(read_pipe(pipe, sender.clone())); }
    if let Some(pipe) = child.stderr.take() { tokio::spawn(read_pipe(pipe, sender.clone())); }
    drop(sender);
    let status = loop {
        tokio::select! {
            result = child.wait() => break result.map_err(|e| e.to_string())?,
            line = logs.recv(), if !logs.is_closed() => if let Some(line) = line {
                let _ = context.app.emit_all("android-log", json!({ "taskId": context.request.id, "line": line }));
            },
        }
    };
    while let Ok(line) = logs.try_recv() {
        let _ = context.app.emit_all("android-log", json!({ "taskId": context.request.id, "line": line }));
    }
    let Some(summary): Option<RunSummary> = fs::read(&files.result).ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok()) else {
        return Ok(Outcome::action("Android 执行结果未确认，请先检查大麦 App 官方订单页，避免重复下单", order_list_url("dm").into()));
    };
    if status.code() != Some(summary.exit_code) {
        return Ok(Outcome::action("Android 进程退出状态与运行摘要不一致，请检查官方订单", order_list_url("dm").into()));
    }
    Ok(summary_outcome(&summary, order_list_url("dm")))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn result_requires_confirmed_submit() {
        let url = order_list_url("dm");
        for outcome in ["order_submitted", "order_pending_payment"] {
            assert_eq!(summary_outcome(&RunSummary { outcome: outcome.into(), exit_code: 0, mode: Some("submit".into()), terminal_reason: None }, url).status, "succeeded");
        }
        assert_eq!(summary_outcome(&RunSummary { outcome: "order_flow_completed".into(), exit_code: 0, mode: Some("submit".into()), terminal_reason: None }, url).status, "needs_action");
        assert_eq!(summary_outcome(&RunSummary { outcome: "terminal_failure".into(), exit_code: 11, mode: Some("submit".into()), terminal_reason: Some("submit_unverified".into()) }, url).status, "needs_action");
        assert_eq!(summary_outcome(&RunSummary { outcome: "retries_exhausted".into(), exit_code: 10, mode: Some("submit".into()), terminal_reason: None }, url).status, "failed");
    }
    #[test]
    fn device_parser_excludes_headers() {
        let items = parse_devices("List of devices attached\nA device product:x model:y\nB unauthorized\n");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].serial, "A");
        assert_eq!(items[1].state, "unauthorized");
    }
}
