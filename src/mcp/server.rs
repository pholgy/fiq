use std::io::{self, BufRead, Write};

use serde_json::{Value, json};

use crate::mcp::handler::handle_tool_call;
use crate::mcp::protocol::{JsonRpcRequest, JsonRpcResponse, ToolCallParams};
use crate::mcp::tools::tool_definitions;

/// Run the MCP JSON-RPC 2.0 server over stdin/stdout (newline-delimited).
pub fn run_mcp_server() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse::error(None, -32700, format!("Parse error: {}", e));
                let _ = write_response(&mut stdout, &resp);
                continue;
            }
        };

        // JSON-RPC 2.0: requests without an id are notifications — never respond
        if request.id.is_none() {
            continue;
        }

        let response = handle_request(&request);
        let _ = write_response(&mut stdout, &response);
    }
}

fn write_response(stdout: &mut io::StdoutLock, response: &JsonRpcResponse) -> io::Result<()> {
    let json = serde_json::to_string(response)?;
    writeln!(stdout, "{}", json)?;
    stdout.flush()
}

fn handle_request(request: &JsonRpcRequest) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => {
            let result = json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {
                        "listChanged": false
                    }
                },
                "serverInfo": {
                    "name": "fiq",
                    "version": env!("CARGO_PKG_VERSION")
                }
            });
            JsonRpcResponse::success(request.id.clone(), result)
        }

        "ping" => JsonRpcResponse::success(request.id.clone(), json!({})),

        "tools/list" => {
            let tools = tool_definitions();
            JsonRpcResponse::success(request.id.clone(), tools)
        }

        "tools/call" => {
            let params: ToolCallParams = match &request.params {
                Some(p) => match serde_json::from_value(p.clone()) {
                    Ok(tc) => tc,
                    Err(e) => {
                        return JsonRpcResponse::error(
                            request.id.clone(),
                            -32602,
                            format!("Invalid params: {}", e),
                        );
                    }
                },
                None => {
                    return JsonRpcResponse::error(
                        request.id.clone(),
                        -32602,
                        "Missing params".to_string(),
                    );
                }
            };

            let tool_result = handle_tool_call(&params.name, &params.arguments);
            match tool_result {
                Ok(result) => {
                    let result_value: Value = serde_json::to_value(&result).unwrap_or(json!(null));
                    JsonRpcResponse::success(request.id.clone(), result_value)
                }
                Err(msg) => JsonRpcResponse::error(request.id.clone(), -32602, msg),
            }
        }

        _ => JsonRpcResponse::error(
            request.id.clone(),
            -32601,
            format!("Method not found: {}", request.method),
        ),
    }
}
