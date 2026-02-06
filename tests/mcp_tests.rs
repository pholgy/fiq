use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn fiq_bin() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_BIN_EXE_fiq"));
    if !path.exists() {
        path = PathBuf::from("target/debug/fiq");
        if cfg!(windows) {
            path.set_extension("exe");
        }
    }
    path
}

fn send_mcp_request(request: &str) -> String {
    let mut child = Command::new(fiq_bin())
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start fiq --mcp");

    {
        let stdin = child.stdin.as_mut().expect("failed to open stdin");
        stdin
            .write_all(request.as_bytes())
            .expect("failed to write to stdin");
        stdin.write_all(b"\n").expect("failed to write newline");
    }

    // Close stdin to signal EOF
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("failed to wait on child");
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn send_mcp_requests(requests: &[&str]) -> Vec<String> {
    let mut child = Command::new(fiq_bin())
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start fiq --mcp");

    {
        let stdin = child.stdin.as_mut().expect("failed to open stdin");
        for req in requests {
            stdin
                .write_all(req.as_bytes())
                .expect("failed to write to stdin");
            stdin.write_all(b"\n").expect("failed to write newline");
        }
    }

    drop(child.stdin.take());
    let output = child.wait_with_output().expect("failed to wait on child");
    let stdout = String::from_utf8_lossy(&output.stdout);

    stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.to_string())
        .collect()
}

#[test]
fn test_mcp_initialize() {
    let response = send_mcp_request(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}"#,
    );

    let parsed: serde_json::Value = serde_json::from_str(response.trim()).expect("invalid JSON");
    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 1);
    assert!(parsed["result"]["serverInfo"]["name"] == "fiq");
    assert!(parsed["result"]["capabilities"]["tools"].is_object());
}

#[test]
fn test_mcp_ping() {
    let response = send_mcp_request(r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#);

    let parsed: serde_json::Value = serde_json::from_str(response.trim()).expect("invalid JSON");
    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 2);
    assert!(parsed["result"].is_object());
}

#[test]
fn test_mcp_tools_list() {
    let response = send_mcp_request(r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#);

    let parsed: serde_json::Value = serde_json::from_str(response.trim()).expect("invalid JSON");
    assert_eq!(parsed["jsonrpc"], "2.0");

    let tools = &parsed["result"]["tools"];
    assert!(tools.is_array());

    let tool_names: Vec<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();

    assert!(tool_names.contains(&"scan_stats"));
    assert!(tool_names.contains(&"find_duplicates"));
    assert!(tool_names.contains(&"search_files"));
    assert!(tool_names.contains(&"organize_files"));
    assert!(tool_names.contains(&"build_index"));
}

#[test]
fn test_mcp_scan_stats() {
    let dir = std::env::current_dir().unwrap();
    let request = format!(
        r#"{{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{{"name":"scan_stats","arguments":{{"directory":"{}","top_n":5}}}}}}"#,
        dir.display().to_string().replace('\\', "\\\\")
    );

    let response = send_mcp_request(&request);
    let parsed: serde_json::Value = serde_json::from_str(response.trim()).expect("invalid JSON");

    assert_eq!(parsed["jsonrpc"], "2.0");
    assert!(parsed["result"]["content"].is_array());

    let text = parsed["result"]["content"][0]["text"]
        .as_str()
        .expect("missing text");
    let stats: serde_json::Value = serde_json::from_str(text).expect("invalid stats JSON");
    assert!(stats["total_files"].is_number());
    assert!(stats["total_size"].is_number());
}

#[test]
fn test_mcp_notification_no_response() {
    let responses = send_mcp_requests(&[
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        r#"{"jsonrpc":"2.0","id":5,"method":"ping"}"#,
    ]);

    // Notification should not produce a response, only the ping should
    assert_eq!(responses.len(), 1);
    let parsed: serde_json::Value = serde_json::from_str(&responses[0]).expect("invalid JSON");
    assert_eq!(parsed["id"], 5);
}

#[test]
fn test_mcp_unknown_method() {
    let response = send_mcp_request(r#"{"jsonrpc":"2.0","id":6,"method":"nonexistent/method"}"#);

    let parsed: serde_json::Value = serde_json::from_str(response.trim()).expect("invalid JSON");
    assert!(parsed["error"].is_object());
    assert_eq!(parsed["error"]["code"], -32601);
}

#[test]
fn test_mcp_unknown_tool() {
    let response = send_mcp_request(
        r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"nonexistent_tool","arguments":{}}}"#,
    );

    let parsed: serde_json::Value = serde_json::from_str(response.trim()).expect("invalid JSON");
    assert!(parsed["error"].is_object());
    assert_eq!(parsed["error"]["code"], -32602);
    assert!(
        parsed["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Unknown tool")
    );
}

#[test]
fn test_mcp_invalid_json() {
    let response = send_mcp_request("this is not json");

    let parsed: serde_json::Value = serde_json::from_str(response.trim()).expect("invalid JSON");
    assert!(parsed["error"].is_object());
    assert_eq!(parsed["error"]["code"], -32700);
}

#[test]
fn test_mcp_full_session() {
    let dir = std::env::current_dir().unwrap();
    let dir_escaped = dir.display().to_string().replace('\\', "\\\\");

    let responses = send_mcp_requests(&[
        // 1. Initialize
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}"#,
        // 2. Notification (no response expected)
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        // 3. List tools
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        // 4. Call scan_stats
        &format!(
            r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"scan_stats","arguments":{{"directory":"{}"}}}}}}"#,
            dir_escaped
        ),
    ]);

    // Should get 3 responses (notification doesn't produce one)
    assert_eq!(responses.len(), 3);

    // Verify initialize response
    let init: serde_json::Value = serde_json::from_str(&responses[0]).expect("invalid JSON");
    assert_eq!(init["id"], 1);
    assert!(init["result"]["serverInfo"]["name"] == "fiq");

    // Verify tools/list response
    let tools: serde_json::Value = serde_json::from_str(&responses[1]).expect("invalid JSON");
    assert_eq!(tools["id"], 2);
    assert!(tools["result"]["tools"].is_array());

    // Verify scan_stats response
    let stats: serde_json::Value = serde_json::from_str(&responses[2]).expect("invalid JSON");
    assert_eq!(stats["id"], 3);
    assert!(stats["result"]["content"].is_array());
}
