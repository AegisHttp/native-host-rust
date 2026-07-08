use fs2::FileExt;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{self, IsTerminal, Read, Write};
use std::process::Command;

#[cfg(target_os = "windows")]
use winreg::enums::*;
#[cfg(target_os = "windows")]
use winreg::RegKey;

fn acquire_gpg_lock() -> File {
    let mut lock_path = env::temp_dir();
    lock_path.push("aegis_http_gpg.lock");
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(&lock_path)
        .unwrap_or_else(|_| panic!("Failed to open lock file at {:?}", lock_path));
    file.lock_exclusive().expect("Failed to acquire GPG Mutex");
    file
}

fn get_gpg_command() -> String {
    #[cfg(target_os = "windows")]
    {
        use std::path::Path;
        let check_paths = [
            r#"C:\Program Files (x86)\GnuPG\bin\gpg.exe"#,
            r#"C:\Program Files\GnuPG\bin\gpg.exe"#,
            r#"C:\Program Files (x86)\Gpg4win\bin\gpg.exe"#,
            r#"C:\Program Files\Gpg4win\bin\gpg.exe"#,
            r#"C:\Program Files (x86)\Gpg4win\gpg.exe"#,
            r#"C:\Program Files\Gpg4win\gpg.exe"#,
        ];
        for p in check_paths.iter() {
            if Path::new(p).exists() {
                return p.to_string();
            }
        }
    }
    "gpg".to_string()
}

fn read_message() -> Option<Value> {
    let mut stdin = io::stdin();
    let mut length_bytes = [0u8; 4];
    if stdin.read_exact(&mut length_bytes).is_err() {
        return None;
    }
    let length = u32::from_ne_bytes(length_bytes) as usize;
    let mut buffer = vec![0u8; length];
    if stdin.read_exact(&mut buffer).is_err() {
        return None;
    }
    serde_json::from_slice(&buffer).ok()
}

fn send_message(msg: &Value) {
    let serialized = serde_json::to_string(msg).unwrap();
    let length = serialized.len() as u32;
    let mut stdout = io::stdout();
    stdout.write_all(&length.to_ne_bytes()).unwrap();
    stdout.write_all(serialized.as_bytes()).unwrap();
    stdout.flush().unwrap();
}

fn send_message_chunked(
    msg_id: &str,
    _action: &str,
    payload_field: &str,
    payload: String,
    mut original_response: serde_json::Map<String, Value>,
) {
    let chunk_size = 800 * 1024;
    if payload.len() < chunk_size {
        original_response.insert(payload_field.to_string(), Value::String(payload));
        original_response.insert("status".to_string(), Value::String("success".to_string()));
        send_message(&Value::Object(original_response));
        return;
    }

    let total = (payload.len() as f64 / chunk_size as f64).ceil() as usize;
    for (i, chunk) in payload.as_bytes().chunks(chunk_size).enumerate() {
        let chunk_str = String::from_utf8_lossy(chunk).to_string();
        send_message(&serde_json::json!({
            "action": "chunk_reply",
            "msg_id": msg_id,
            "index": i,
            "total": total,
            "data": chunk_str
        }));
    }

    original_response.insert("chunked_reply".to_string(), Value::Bool(true));
    original_response.insert("status".to_string(), Value::String("success".to_string()));
    send_message(&Value::Object(original_response));
}

fn sign_challenge(challenge: &str, email: &str) -> Result<String, String> {
    let _lock = acquire_gpg_lock();
    let mut child = Command::new(get_gpg_command())
        .args(["--clear-sign", "--local-user", email])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(challenge.as_bytes())
        .map_err(|e| e.to_string())?;
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(format!(
            "GPG Sign Error: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn decrypt_payload(encrypted_text: &str) -> Result<String, String> {
    let _lock = acquire_gpg_lock();
    let mut child = Command::new(get_gpg_command())
        .args(["--decrypt", "--trust-model", "always"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(encrypted_text.as_bytes())
            .map_err(|e| e.to_string())?;
    } else {
        return Err("Failed to get stdin for GPG command".to_string());
    }
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(format!(
            "GPG Decrypt Error: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

static TEMP_FILE_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn get_unique_temp_file() -> std::path::PathBuf {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let counter = TEMP_FILE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    std::env::temp_dir().join(format!("aegis_key_{}_{}.pub", now, counter))
}

fn encrypt_payload(recipient: &str, public_key: &str, plaintext: &str) -> Result<String, String> {
    if !public_key.is_empty() {
        let temp_file_path = get_unique_temp_file();
        std::fs::write(&temp_file_path, public_key.as_bytes()).map_err(|e| e.to_string())?;

        let mut child = Command::new(get_gpg_command())
            .args([
                "--encrypt",
                "--armor",
                "--trust-model",
                "always",
                "--recipient-file",
                temp_file_path.to_str().unwrap(),
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                let _ = std::fs::remove_file(&temp_file_path);
                e.to_string()
            })?;

        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(plaintext.as_bytes())
            .map_err(|e| {
                let _ = std::fs::remove_file(&temp_file_path);
                e.to_string()
            })?;

        let output = child.wait_with_output().map_err(|e| {
            let _ = std::fs::remove_file(&temp_file_path);
            e.to_string()
        })?;

        let _ = std::fs::remove_file(&temp_file_path);

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(format!(
                "GPG Encrypt Error: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    } else {
        let mut child = Command::new(get_gpg_command())
            .args([
                "--encrypt",
                "--armor",
                "--trust-model",
                "always",
                "--recipient",
                recipient,
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(plaintext.as_bytes())
            .map_err(|e| e.to_string())?;
        let output = child.wait_with_output().map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(format!(
                "GPG Encrypt Error: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }
}

struct RawKeyBlock {
    fingerprint: String,
    emails: Vec<String>,
    has_encrypt: bool,
}

fn list_keys() -> Value {
    let output = Command::new(get_gpg_command())
        .args(["--list-secret-keys", "--with-colons"])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let mut keys = Vec::new();
            let mut current_key: Option<RawKeyBlock> = None;
            let mut last_was_sec = false;

            for line in stdout.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.is_empty() {
                    continue;
                }

                match parts[0] {
                    "sec" => {
                        if let Some(k) = current_key.take() {
                            keys.push(k);
                        }
                        let mut has_encrypt = false;
                        if parts.len() > 11 {
                            let caps = parts[11].to_lowercase();
                            if caps.contains('e') {
                                has_encrypt = true;
                            }
                        }
                        current_key = Some(RawKeyBlock {
                            fingerprint: String::new(),
                            emails: Vec::new(),
                            has_encrypt,
                        });
                        last_was_sec = true;
                    }
                    "ssb" => {
                        last_was_sec = false;
                        if let Some(ref mut k) = current_key {
                            if parts.len() > 11 {
                                let caps = parts[11].to_lowercase();
                                if caps.contains('e') {
                                    k.has_encrypt = true;
                                }
                            }
                        }
                    }
                    "fpr" => {
                        if let Some(ref mut k) = current_key {
                            if parts.len() > 9 && !parts[9].is_empty() {
                                if last_was_sec {
                                    k.fingerprint = parts[9].to_string();
                                }
                            }
                        }
                        last_was_sec = false;
                    }
                    "uid" => {
                        if let Some(ref mut k) = current_key {
                            if parts.len() > 9 {
                                let uid_field = parts[9];
                                if let Some(start) = uid_field.find('<') {
                                    if let Some(end) = uid_field.find('>') {
                                        let email = uid_field[start + 1..end].to_string();
                                        if !k.emails.contains(&email) {
                                            k.emails.push(email);
                                        }
                                    }
                                }
                            }
                        }
                        last_was_sec = false;
                    }
                    _ => {
                        if parts[0] != "grp" {
                            last_was_sec = false;
                        }
                    }
                }
            }
            if let Some(k) = current_key {
                keys.push(k);
            }

            let mut emails = Vec::new();
            let mut key_list = Vec::new();
            for k in keys {
                let pub_key = match Command::new(get_gpg_command())
                    .args(["--export", "--armor", &k.fingerprint])
                    .output()
                {
                    Ok(out) if out.status.success() => {
                        String::from_utf8_lossy(&out.stdout).to_string()
                    }
                    _ => String::new(),
                };
                for email in &k.emails {
                    if !emails.contains(email) {
                        emails.push(email.clone());
                    }
                    key_list.push(serde_json::json!({
                        "email": email,
                        "fingerprint": k.fingerprint,
                        "has_encrypt": k.has_encrypt,
                        "public_key": pub_key
                    }));
                }
            }

            serde_json::json!({
                "action": "list-keys",
                "status": "success",
                "emails": emails,
                "keys": key_list
            })
        }
        _ => {
            serde_json::json!({ "action": "list-keys", "status": "error", "error": "Failed to list keys" })
        }
    }
}

fn add_subkey(target: &str, algo: &str, expire: &str) -> Result<String, String> {
    let _lock = acquire_gpg_lock();
    let output = Command::new(get_gpg_command())
        .args([
            "--batch",
            "--no-tty",
            "--quick-add-key",
            target,
            algo,
            "encrypt",
            expire,
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok("Subkey added successfully".to_string())
    } else {
        Err(format!(
            "GPG Add Subkey Error: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[cfg(target_os = "windows")]
fn install_native_host() {
    let host_name = "com.aegis.http.gpg";

    // Get absolute path of this executable
    let exe_path = env::current_exe().expect("Failed to get current executable path");
    let exe_path_str = exe_path.to_string_lossy().replace("\\", "\\\\");

    let ext_id = option_env!("CHROME_EXTENSION_ID").unwrap_or("lappbcambkogfmigiphapgjcglafcfnd");
    // Generate manifest.json content
    let manifest_content = format!(
        r#"{{
  "name": "{}",
  "description": "Aegis Http Native Host Daemon",
  "path": "{}",
  "type": "stdio",
  "allowed_origins": [
    "chrome-extension://{}/"
  ]
}}"#,
        host_name, exe_path_str, ext_id
    );

    // Save manifest right next to the executable
    let manifest_path = exe_path.with_file_name(format!("{}.json", host_name));
    let mut manifest_file = File::create(&manifest_path).expect("Failed to create manifest file");
    manifest_file
        .write_all(manifest_content.as_bytes())
        .expect("Failed to write manifest file");

    let manifest_path_str = manifest_path.to_string_lossy().to_string();

    // Register for Chrome
    println!("Registering Native Host for Chrome...");
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let chrome_path = format!(
        "Software\\Google\\Chrome\\NativeMessagingHosts\\{}",
        host_name
    );
    let (chrome_key, _) = hkcu
        .create_subkey(&chrome_path)
        .expect("Failed to open Chrome registry");
    chrome_key
        .set_value("", &manifest_path_str)
        .expect("Failed to write to Chrome registry");

    // Register for Edge
    println!("Registering Native Host for Edge...");
    let edge_path = format!(
        "Software\\Microsoft\\Edge\\NativeMessagingHosts\\{}",
        host_name
    );
    let (edge_key, _) = hkcu
        .create_subkey(&edge_path)
        .expect("Failed to open Edge registry");
    edge_key
        .set_value("", &manifest_path_str)
        .expect("Failed to write to Edge registry");

    // Register for Firefox (Firefox requires allowed_extensions instead of origins, so we just register the base manifest although it's originally chrome.json. We will write a specific Firefox manifest)
    let firefox_manifest_content = format!(
        r#"{{
  "name": "{}",
  "description": "Aegis Http Native Host Daemon",
  "path": "{}",
  "type": "stdio",
  "allowed_extensions": [
    "aegis-http@aegishttp.com"
  ]
}}"#,
        host_name, exe_path_str
    );

    let firefox_manifest_path = exe_path.with_file_name(format!("{}-firefox.json", host_name));
    let mut firefox_manifest_file =
        File::create(&firefox_manifest_path).expect("Failed to create Firefox manifest file");
    firefox_manifest_file
        .write_all(firefox_manifest_content.as_bytes())
        .expect("Failed to write Firefox manifest file");
    let firefox_manifest_path_str = firefox_manifest_path.to_string_lossy().to_string();

    println!("Registering Native Host for Firefox...");
    let firefox_path = format!("Software\\Mozilla\\NativeMessagingHosts\\{}", host_name);
    let (firefox_key, _) = hkcu
        .create_subkey(&firefox_path)
        .expect("Failed to open Firefox registry");
    firefox_key
        .set_value("", &firefox_manifest_path_str)
        .expect("Failed to write to Firefox registry");

    println!("✅ Aegis Http Native Host successfully installed to the Windows Registry.");
    println!("\nYou can now close this window.");
}

#[cfg(target_os = "macos")]
fn install_native_host() {
    let host_name = "com.aegis.http.gpg";
    let exe_path = env::current_exe().expect("Failed to get current executable path");
    let exe_path_str = exe_path.to_string_lossy().to_string();

    let ext_id = option_env!("CHROME_EXTENSION_ID").unwrap_or("lappbcambkogfmigiphapgjcglafcfnd");
    let manifest_content = format!(
        r#"{{
  "name": "{}",
  "description": "Aegis Http Native Host Daemon",
  "path": "{}",
  "type": "stdio",
  "allowed_origins": [
    "chrome-extension://{}/"
  ]
}}"#,
        host_name, exe_path_str, ext_id
    );

    let firefox_manifest_content = format!(
        r#"{{
  "name": "{}",
  "description": "Aegis Http Native Host Daemon",
  "path": "{}",
  "type": "stdio",
  "allowed_extensions": [
    "aegis-http@aegishttp.com"
  ]
}}"#,
        host_name, exe_path_str
    );

    let home_dir = env::var("HOME").expect("Failed to get HOME environment variable");

    let targets = vec![
        (
            format!(
                "{}/Library/Application Support/Google/Chrome/NativeMessagingHosts/{}.json",
                home_dir, host_name
            ),
            &manifest_content,
        ),
        (
            format!(
                "{}/Library/Application Support/Microsoft Edge/NativeMessagingHosts/{}.json",
                home_dir, host_name
            ),
            &manifest_content,
        ),
        (
            format!(
                "{}/Library/Application Support/Mozilla/NativeMessagingHosts/{}.json",
                home_dir, host_name
            ),
            &firefox_manifest_content,
        ),
    ];

    for (target_path, content) in targets {
        let path = std::path::Path::new(&target_path);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut file = File::create(path)
            .unwrap_or_else(|_| panic!("Failed to create manifest at {}", target_path));
        file.write_all(content.as_bytes())
            .expect("Failed to write to manifest");
        println!("Registered host at: {}", target_path);
    }

    println!("✅ Aegis Http Native Host successfully installed to macOS paths.");
    println!("\nYou can now close this window.");
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn install_native_host() {
    println!("The self-installer logic is currently only implemented for Windows and macOS.");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // Auto install if run directly from a terminal or double-clicked on Windows
    if args.iter().any(|arg| arg == "--install") || io::stdin().is_terminal() {
        install_native_host();

        // Keep window open if double clicked
        if io::stdin().is_terminal() {
            println!("Press [ENTER] to exit...");
            let mut s = String::new();
            io::stdin().read_line(&mut s).unwrap_or(0);
        }
        return;
    }

    let mut chunk_store: HashMap<String, HashMap<usize, String>> = HashMap::new();

    loop {
        if let Some(msg) = read_message() {
            let msg_id = msg
                .get("msg_id")
                .and_then(|v| v.as_str())
                .unwrap_or("default_id")
                .to_string();
            let action = msg.get("action").and_then(|v| v.as_str()).unwrap_or("");

            if action == "chunk" {
                let index = msg.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let data = msg
                    .get("data")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                chunk_store
                    .entry(msg_id.clone())
                    .or_insert_with(HashMap::new)
                    .insert(index, data);
                send_message(&serde_json::json!({ "status": "chunk_received", "msg_id": msg_id }));
                continue;
            }

            let mut final_text = String::new();
            if msg
                .get("chunked")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                if let Some(chunks_map) = chunk_store.remove(&msg_id) {
                    for i in 0..chunks_map.len() {
                        if let Some(c) = chunks_map.get(&i) {
                            final_text.push_str(c);
                        }
                    }
                }
            } else {
                final_text = msg
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
            }

            let mut map = serde_json::Map::new();
            map.insert("action".to_string(), Value::String(action.to_string()));
            map.insert("msg_id".to_string(), Value::String(msg_id.clone()));

            let mut result_payload = None;
            let mut result_field = "";

            match action {
                "list-keys" => {
                    let mut res = list_keys();
                    if let Value::Object(ref mut m) = res {
                        m.insert("msg_id".to_string(), Value::String(msg_id));
                    }
                    send_message(&res);
                    continue;
                }

                "sign" => {
                    let email = msg.get("email").and_then(|v| v.as_str()).unwrap_or("");
                    match sign_challenge(&final_text, email) {
                        Ok(res) => {
                            result_payload = Some(res);
                            result_field = "signature";
                        }
                        Err(e) => {
                            map.insert("error".to_string(), Value::String(e));
                        }
                    }
                }
                "encrypt" => {
                    let email = msg.get("email").and_then(|v| v.as_str()).unwrap_or("");
                    let public_key = msg.get("public_key").and_then(|v| v.as_str()).unwrap_or("");
                    match encrypt_payload(email, public_key, &final_text) {
                        Ok(res) => {
                            result_payload = Some(res);
                            result_field = "encrypted";
                        }
                        Err(e) => {
                            map.insert("error".to_string(), Value::String(e));
                        }
                    }
                }
                "decrypt" => match decrypt_payload(&final_text) {
                    Ok(res) => {
                        result_payload = Some(res);
                        result_field = "decrypted";
                    }
                    Err(e) => {
                        map.insert("error".to_string(), Value::String(e));
                    }
                },
                "add-subkey" => {
                    let fingerprint = msg
                        .get("fingerprint")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let email = msg.get("email").and_then(|v| v.as_str()).unwrap_or("");
                    let algo = msg
                        .get("algo")
                        .and_then(|v| v.as_str())
                        .unwrap_or("rsa3072");
                    let expire = msg.get("expire").and_then(|v| v.as_str()).unwrap_or("0");
                    let target = if !fingerprint.is_empty() {
                        fingerprint
                    } else {
                        email
                    };
                    match add_subkey(target, algo, expire) {
                        Ok(res) => {
                            result_payload = Some(res);
                            result_field = "message";
                        }
                        Err(e) => {
                            map.insert("error".to_string(), Value::String(e));
                        }
                    }
                }
                _ => {
                    map.insert(
                        "error".to_string(),
                        Value::String("Unknown action".to_string()),
                    );
                }
            }

            if let Some(payload) = result_payload {
                send_message_chunked(&msg_id, action, result_field, payload, map);
            } else {
                map.insert("status".to_string(), Value::String("error".to_string()));
                send_message(&Value::Object(map));
            }
        } else {
            break; // EOF or err
        }
    }
}
