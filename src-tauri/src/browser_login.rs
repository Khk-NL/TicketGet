use chromiumoxide::{browser::BrowserConfig, cdp::browser_protocol::network::Cookie, Browser};
use futures::StreamExt;
use std::path::PathBuf;
use tokio::sync::Mutex;

pub struct LoginManager(pub Mutex<Option<Browser>>);

impl Default for LoginManager {
    fn default() -> Self {
        Self(Mutex::new(None))
    }
}

fn browser_executables(choice: &str) -> Result<Vec<PathBuf>, String> {
    if !matches!(choice, "auto" | "edge" | "chrome") {
        return Err("浏览器选择无效".into());
    }
    let mut paths = Vec::new();
    #[cfg(target_os = "windows")]
    {
        for (kind, variable, suffix) in [
            ("edge", "PROGRAMFILES(X86)", "Microsoft/Edge/Application/msedge.exe"),
            ("edge", "PROGRAMFILES", "Microsoft/Edge/Application/msedge.exe"),
            ("edge", "LOCALAPPDATA", "Microsoft/Edge/Application/msedge.exe"),
            ("chrome", "PROGRAMFILES", "Google/Chrome/Application/chrome.exe"),
            ("chrome", "PROGRAMFILES(X86)", "Google/Chrome/Application/chrome.exe"),
            ("chrome", "LOCALAPPDATA", "Google/Chrome/Application/chrome.exe"),
        ] {
            if choice != "auto" && choice != kind { continue; }
            if let Some(root) = std::env::var_os(variable) {
                let path = PathBuf::from(root).join(suffix);
                if path.is_file() {
                    paths.push(path);
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    if choice != "auto" {
        return Err("此平台暂不支持指定浏览器，请选择自动".into());
    }
    Ok(paths)
}

fn cookie_header(cookies: &[Cookie]) -> Result<String, String> {
    let selected: Vec<String> = cookies
        .iter()
        .filter(|cookie| {
            let domain = cookie.domain.trim_start_matches('.').to_ascii_lowercase();
            (domain == "damai.cn" || domain == "mtop.damai.cn")
                && cookie.path == "/"
                && !cookie.name.chars().any(|ch| matches!(ch, ';' | '=' | '\r' | '\n'))
                && !cookie.value.chars().any(|ch| matches!(ch, ';' | '\r' | '\n'))
        })
        .map(|cookie| format!("{}={}", cookie.name, cookie.value))
        .collect();
    if !selected.iter().any(|pair| pair.starts_with("_m_h5_tk=")) {
        return Err("尚未取得大麦 H5 登录凭据。请在打开的浏览器完成登录并访问活动页，然后重试读取。".into());
    }
    let header = selected.join("; ");
    if header.len() > 32768 {
        return Err("大麦 Cookie 超过 32 KB，无法保存".into());
    }
    Ok(header)
}

#[tauri::command]
pub async fn start_damai_browser_login(
    manager: tauri::State<'_, LoginManager>,
    browser_choice: String,
) -> Result<(), String> {
    let mut session = manager.0.lock().await;
    if session.is_some() {
        return Err("登录浏览器已打开，请完成读取或取消".into());
    }
    let paths = browser_executables(&browser_choice)?;
    #[cfg(target_os = "windows")]
    if paths.is_empty() {
        let name = match browser_choice.as_str() { "edge" => "Edge", "chrome" => "Chrome", _ => "Edge 或 Chrome" };
        return Err(format!("未找到{name}，请检查浏览器安装路径"));
    }
    let candidates: Vec<Option<PathBuf>> = if paths.is_empty() { vec![None] } else { paths.into_iter().map(Some).collect() };
    let mut failures = Vec::new();
    let mut launched = None;
    for path in candidates {
        let mut builder = BrowserConfig::builder().with_head().incognito();
        if let Some(path) = &path { builder = builder.chrome_executable(path); }
        let config = builder.build().map_err(|error| format!("浏览器配置失败：{error}"))?;
        match Browser::launch(config).await {
            Ok(value) => { launched = Some(value); break; }
            Err(error) => {
                let name = path.as_ref().and_then(|value| value.file_name()).and_then(|value| value.to_str()).unwrap_or("默认浏览器");
                failures.push(format!("{name}: {error}"));
            }
        }
    }
    let (browser, mut handler) = launched.ok_or_else(|| format!("浏览器启动失败：{}", failures.join("；")))?;
    tokio::spawn(async move { while handler.next().await.is_some() {} });
    if browser.new_page("https://m.damai.cn/").await.is_err() {
        return Err("大麦登录页面打开失败，请检查网络连接".into());
    }
    *session = Some(browser);
    Ok(())
}

#[tauri::command]
pub async fn read_damai_browser_cookie(
    manager: tauri::State<'_, LoginManager>,
) -> Result<String, String> {
    let mut session = manager.0.lock().await;
    let browser = session.as_mut().ok_or("请先打开登录浏览器")?;
    let cookies = browser.get_cookies().await.map_err(|_| "浏览器已关闭或无法读取 Cookie，请重新打开".to_string())?;
    let header = cookie_header(&cookies)?;
    let _ = browser.close().await;
    *session = None;
    Ok(header)
}

#[tauri::command]
pub async fn cancel_damai_browser_login(
    manager: tauri::State<'_, LoginManager>,
) -> Result<(), String> {
    let mut session = manager.0.lock().await;
    if let Some(mut browser) = session.take() {
        let _ = browser.close().await;
    }
    Ok(())
}
