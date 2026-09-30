use aes_gcm::{aead::{Aead, AeadCore, OsRng}, Aes256Gcm, KeyInit, Nonce};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::{fs, io::Write, path::Path};
use tauri::AppHandle;
use uuid::Uuid;

const HEADER: &[u8] = b"TICKETGET1";

fn master_key() -> Result<[u8; 32], String> {
    let entry = keyring::Entry::new("TicketGet", "local-master-key-v1")
        .map_err(|_| "系统凭据存储不可用")?;
    let encoded = match entry.get_password() {
        Ok(value) => value,
        Err(keyring::Error::NoEntry) => {
            let key = Aes256Gcm::generate_key(OsRng);
            let value = STANDARD.encode(key);
            entry.set_password(&value).map_err(|_| "无法写入系统凭据存储")?;
            value
        }
        Err(_) => return Err("无法读取系统凭据存储".into()),
    };
    let bytes = STANDARD.decode(encoded).map_err(|_| "系统凭据格式无效")?;
    bytes.try_into().map_err(|_| "系统凭据长度无效".into())
}

fn seal(key: &[u8; 32], plain: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "凭据加密初始化失败")?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let encrypted = cipher.encrypt(&nonce, plain).map_err(|_| "凭据加密失败")?;
    let mut result = Vec::with_capacity(HEADER.len() + nonce.len() + encrypted.len());
    result.extend_from_slice(HEADER);
    result.extend_from_slice(&nonce);
    result.extend_from_slice(&encrypted);
    Ok(result)
}

fn unseal(key: &[u8; 32], bytes: &[u8]) -> Result<Vec<u8>, String> {
    if !bytes.starts_with(HEADER) || bytes.len() < HEADER.len() + 12 + 16 {
        return Err("凭据文件格式无效".into());
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "凭据解密初始化失败")?;
    let nonce = Nonce::from_slice(&bytes[HEADER.len()..HEADER.len() + 12]);
    cipher.decrypt(nonce, &bytes[HEADER.len() + 12..]).map_err(|_| "凭据解密失败，请检查系统凭据存储".into())
}

fn write_secure(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let directory = path.parent().ok_or("凭据目录不可用")?;
    fs::create_dir_all(directory).map_err(|_| "无法创建凭据目录")?;
    let temp = directory.join(format!(".{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)] {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).map_err(|_| "无法设置凭据目录权限")?;
            options.mode(0o600);
        }
        let mut file = options.open(&temp).map_err(|_| "无法创建凭据文件")?;
        file.write_all(bytes).and_then(|_| file.sync_all()).map_err(|_| "凭据写入失败")?;
        drop(file);
        fs::rename(&temp, path).map_err(|_| "凭据保存失败，原数据已保留".to_string())
    })();
    if result.is_err() { let _ = fs::remove_file(&temp); }
    result
}

pub fn read_secret(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("凭据文件读取失败".into()),
    };
    if bytes.len() > 2_097_152 { return Err("凭据文件过大".into()); }
    unseal(&master_key()?, &bytes).map(Some)
}

pub fn write_secret(path: &Path, plain: &[u8]) -> Result<(), String> {
    if plain.len() > 2_097_152 { return Err("凭据过大".into()); }
    let encrypted = seal(&master_key()?, plain)?;
    write_secure(path, &encrypted)
}

fn account_path(app: &AppHandle, id: &str) -> Result<std::path::PathBuf, String> {
    Uuid::parse_str(id).map_err(|_| "账号 ID 无效")?;
    Ok(app.path_resolver().app_data_dir().ok_or("应用数据目录不可用")?
        .join("accounts").join(format!("{id}.enc")))
}

#[tauri::command]
pub fn put_account_credential(app: AppHandle, id: String, cookie: String) -> Result<(), String> {
    if cookie.is_empty() || cookie.len() > 32768 || cookie.chars().any(char::is_control) {
        return Err("Cookie 格式无效".into());
    }
    write_secret(&account_path(&app, &id)?, cookie.as_bytes())
}

#[tauri::command]
pub fn get_account_credential(app: AppHandle, id: String) -> Result<String, String> {
    let bytes = read_secret(&account_path(&app, &id)?)?.ok_or("账号凭据不存在")?;
    String::from_utf8(bytes).map_err(|_| "账号凭据编码无效".into())
}

#[tauri::command]
pub fn delete_account_credential(app: AppHandle, id: String) -> Result<(), String> {
    match fs::remove_file(account_path(&app, &id)?) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("账号凭据删除失败".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_round_trip_and_tamper_detection() {
        let key = [7_u8; 32];
        let plain = b"session=private";
        let mut sealed = seal(&key, plain).unwrap();
        assert!(!sealed.windows(plain.len()).any(|part| part == plain));
        assert_eq!(unseal(&key, &sealed).unwrap(), plain);
        *sealed.last_mut().unwrap() ^= 1;
        assert!(unseal(&key, &sealed).is_err());
    }
}
