use std::env;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use serde_json::{json, Value};

/// Helper function to build a native messaging payload.
/// 4-bytes representing the length in native byte order, followed by the JSON string.
fn build_native_message(msg: Value) -> Vec<u8> {
    let serialized = msg.to_string();
    let length = serialized.len() as u32;
    let mut buffer = Vec::new();
    buffer.extend_from_slice(&length.to_ne_bytes());
    buffer.extend_from_slice(serialized.as_bytes());
    buffer
}

/// Helper function to parse a native messaging payload from standard output.
fn read_native_message(output: &[u8]) -> Option<Value> {
    if output.len() < 4 {
        return None;
    }
    let mut len_bytes = [0u8; 4];
    len_bytes.copy_from_slice(&output[0..4]);
    let length = u32::from_ne_bytes(len_bytes) as usize;
    if output.len() < 4 + length {
        return None;
    }
    let json_bytes = &output[4..4 + length];
    serde_json::from_slice(json_bytes).ok()
}

#[test]
fn test_unknown_action_returns_error() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_aegis-host"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn native-host-rust");

    let msg = json!({
        "action": "unknown_action_test",
        "msg_id": "req-123"
    });

    let input = build_native_message(msg);
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&input).expect("Failed to write to stdin");
    }

    let mut output = Vec::new();
    if let Some(mut stdout) = child.stdout.take() {
        stdout.read_to_end(&mut output).expect("Failed to read from stdout");
    }

    child.wait().expect("Child process wasn't running");

    let response = read_native_message(&output).expect("Failed to parse the response message");
    
    // Validate the JSON fields
    assert_eq!(response["action"], "unknown_action_test");
    assert_eq!(response["msg_id"], "req-123");
    assert_eq!(response["status"], "error");
    assert_eq!(response["error"], "Unknown action");
}

#[test]
fn test_list_keys_isolated_gpg() {
    // Create an isolated GnuPG home directory so we don't mess up or rely on the host's keys.
    let temp_dir = env::temp_dir().join("gpg_test_dir_rust");
    let _ = std::fs::remove_dir_all(&temp_dir); // clean up from old tests if existed
    std::fs::create_dir_all(&temp_dir).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_aegis-host"))
        .env("GNUPGHOME", &temp_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn native-host-rust");

    let msg = json!({
        "action": "list-keys",
        "msg_id": "req-456"
    });

    let input = build_native_message(msg);
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&input).expect("Failed to write to stdin");
    }

    let mut output = Vec::new();
    if let Some(mut stdout) = child.stdout.take() {
        stdout.read_to_end(&mut output).expect("Failed to read from stdout");
    }

    child.wait().expect("Child process wasn't running");
    
    let response = read_native_message(&output).expect("Failed to parse the response message");
    
    assert_eq!(response["action"], "list-keys");
    assert_eq!(response["msg_id"], "req-456");
    assert_eq!(response["status"], "success");
    
    // There shouldn't be any keys in this isolated gnupghome
    if let Some(emails) = response["emails"].as_array() {
        assert_eq!(emails.len(), 0, "Emails array should be empty in empty gpg dir");
    } else {
        panic!("Emails field missing or not an array");
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_chunk_reassembly() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_aegis-host"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn native-host-rust");

    // Send chunk 0
    let chunk0 = json!({
        "action": "chunk",
        "msg_id": "req-chunk",
        "index": 0,
        "data": "Hello "
    });
    
    // Send chunk 1
    let chunk1 = json!({
        "action": "chunk",
        "msg_id": "req-chunk",
        "index": 1,
        "data": "World!"
    });
    
    // Final action relying on chunked text
    let final_action = json!({
        "action": "sign",
        "email": "nonexistent@email.com",
        "msg_id": "req-chunk",
        "chunked": true
    });

    let mut input = Vec::new();
    input.extend(build_native_message(chunk0));
    input.extend(build_native_message(chunk1));
    input.extend(build_native_message(final_action));

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&input).expect("Failed to write to stdin");
    }

    let mut output = Vec::new();
    if let Some(mut stdout) = child.stdout.take() {
        stdout.read_to_end(&mut output).expect("Failed to read from stdout");
    }

    child.wait().expect("Child process wasn't running");

    // We should receive:
    // 1. status: chunk_received (chunk0)
    // 2. status: chunk_received (chunk1)
    // 3. status: error (import-key with "HelloWorld!")
    
    // Let's parse multiple messages from the output buffer
    let mut current_offset = 0;
    let mut messages_received = 0;
    
    while current_offset < output.len() {
        if current_offset + 4 > output.len() { break; }
        
        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(&output[current_offset..current_offset+4]);
        let length = u32::from_ne_bytes(len_bytes) as usize;
        
        if current_offset + 4 + length > output.len() { break; }
        
        let json_bytes = &output[current_offset+4..current_offset+4+length];
        let msg: Value = serde_json::from_slice(json_bytes).unwrap_or(json!({}));
        
        match messages_received {
            0 => {
                assert_eq!(msg["status"], "chunk_received");
                assert_eq!(msg["msg_id"], "req-chunk");
            },
            1 => {
                assert_eq!(msg["status"], "chunk_received");
                assert_eq!(msg["msg_id"], "req-chunk");
            },
            2 => {
                assert_eq!(msg["action"], "sign");
                assert_eq!(msg["msg_id"], "req-chunk");
                assert_eq!(msg["status"], "error");
                // Contains error about GPG sign failing because nonexistent@email.com is not found
            },
            _ => ()
        }
        
        current_offset += 4 + length;
        messages_received += 1;
    }
    
    assert_eq!(messages_received, 3);
}

#[test]
fn test_subkey_detection_and_generation() {
    let temp_dir = env::temp_dir().join("gpg_test_subkey_generation");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    // 1. Generate a primary key (sign-only)
    let batch_input = "Key-Type: RSA\n\
                       Key-Length: 2048\n\
                       Key-Usage: sign,cert\n\
                       Name-Real: Test User\n\
                       Name-Email: test@aegistest.local\n\
                       Expire-Date: 0\n\
                       %no-protection\n\
                       %commit\n";

    let mut child_gen = Command::new("gpg")
        .env("GNUPGHOME", &temp_dir)
        .args(["--batch", "--generate-key"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn gpg key gen");

    if let Some(mut stdin) = child_gen.stdin.take() {
        stdin.write_all(batch_input.as_bytes()).unwrap();
    }
    
    let gen_out = child_gen.wait_with_output().expect("Failed to wait on key gen");
    assert!(gen_out.status.success(), "GPG key generation failed: {}", String::from_utf8_lossy(&gen_out.stderr));

    // 2. Call list-keys and verify has_encrypt is false
    let mut child_host = Command::new(env!("CARGO_BIN_EXE_aegis-host"))
        .env("GNUPGHOME", &temp_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn native host");

    let list_msg = json!({
        "action": "list-keys",
        "msg_id": "list-1"
    });

    let input = build_native_message(list_msg);
    if let Some(mut stdin) = child_host.stdin.take() {
        stdin.write_all(&input).unwrap();
    }

    let mut output = Vec::new();
    if let Some(mut stdout) = child_host.stdout.take() {
        stdout.read_to_end(&mut output).unwrap();
    }
    child_host.wait().unwrap();

    let response = read_native_message(&output).expect("Failed to parse list-keys response");
    assert_eq!(response["status"], "success");
    
    let keys = response["keys"].as_array().expect("keys should be an array");
    assert_eq!(keys.len(), 1);
    let key_obj = &keys[0];
    assert_eq!(key_obj["email"], "test@aegistest.local");
    assert_eq!(key_obj["has_encrypt"], false, "Initial key should have no encryption subkey");
    let fingerprint = key_obj["fingerprint"].as_str().expect("Fingerprint should be a string").to_string();

    // 3. Call add-subkey to create an encryption subkey
    let mut child_host = Command::new(env!("CARGO_BIN_EXE_aegis-host"))
        .env("GNUPGHOME", &temp_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn native host");

    let add_msg = json!({
        "action": "add-subkey",
        "msg_id": "add-1",
        "fingerprint": fingerprint,
        "algo": "rsa2048",
        "expire": "0"
    });

    let input = build_native_message(add_msg);
    if let Some(mut stdin) = child_host.stdin.take() {
        stdin.write_all(&input).unwrap();
    }

    let mut output = Vec::new();
    if let Some(mut stdout) = child_host.stdout.take() {
        stdout.read_to_end(&mut output).unwrap();
    }
    child_host.wait().unwrap();
    let response = read_native_message(&output).expect("Failed to parse add-subkey response");
    assert_eq!(response["status"], "success");

    // 4. Call list-keys again and verify has_encrypt is now true
    let mut child_host = Command::new(env!("CARGO_BIN_EXE_aegis-host"))
        .env("GNUPGHOME", &temp_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn native host");

    let list_msg2 = json!({
        "action": "list-keys",
        "msg_id": "list-2"
    });

    let input = build_native_message(list_msg2);
    if let Some(mut stdin) = child_host.stdin.take() {
        stdin.write_all(&input).unwrap();
    }

    let mut output = Vec::new();
    if let Some(mut stdout) = child_host.stdout.take() {
        stdout.read_to_end(&mut output).unwrap();
    }
    child_host.wait().unwrap();

    let response = read_native_message(&output).expect("Failed to parse list-keys response 2");
    assert_eq!(response["status"], "success");
    let keys2 = response["keys"].as_array().expect("keys should be an array 2");
    assert_eq!(keys2.len(), 1);
    assert_eq!(keys2[0]["has_encrypt"], true, "Subkey should now be created and detected");

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}
