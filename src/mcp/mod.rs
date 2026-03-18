pub mod protocol;
pub mod tools;
pub mod handlers;

use anyhow::Result;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use crate::config::AespConfig;
use crate::storage::Storage;

pub async fn serve(storage: Storage, config: AespConfig, project_root: PathBuf) -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);

    let server_info = protocol::ServerInfo {
        name: "aesp".to_string(),
        version: "0.1.0".to_string(),
    };

    eprintln!("AESP MCP server v0.1.0 starting for: {}", project_root.display());

    loop {
        let message = match read_mcp_message(&mut reader).await {
            Ok(Some(msg)) => msg,
            Ok(None) => break, // EOF
            Err(e) => {
                eprintln!("AESP: error reading message: {}", e);
                continue;
            }
        };

        let trimmed = message.trim();
        if trimmed.is_empty() {
            continue;
        }

        eprintln!("AESP RECV: {}", &trimmed[..trimmed.len().min(300)]);

        match serde_json::from_str::<protocol::JsonRpcRequest>(trimmed) {
            Ok(request) => {
                // Notifications have null/missing id and no response expected
                let is_notification = request.id.is_null()
                    || request.method.starts_with("notifications/");

                let response = handle_request(
                    &request, &storage, &config, &project_root, &server_info,
                );

                if is_notification {
                    if let Err(e) = &response {
                        eprintln!("AESP: notification {} error: {}", request.method, e);
                    }
                    continue;
                }

                match response {
                    Ok(resp) => {
                        write_mcp_message(&mut stdout, &resp).await?;
                    }
                    Err(e) => {
                        let err_resp = protocol::JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request.id.clone(),
                            result: None,
                            error: Some(protocol::JsonRpcError {
                                code: -32603,
                                message: e.to_string(),
                                data: None,
                            }),
                        };
                        write_mcp_message(&mut stdout, &err_resp).await?;
                    }
                }
            }
            Err(e) => {
                eprintln!("AESP: JSON parse error: {}", e);
                let error_response = protocol::JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: serde_json::Value::Null,
                    result: None,
                    error: Some(protocol::JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                        data: None,
                    }),
                };
                write_mcp_message(&mut stdout, &error_response).await?;
            }
        }
    }

    eprintln!("AESP MCP server shutting down");
    Ok(())
}

/// Read an MCP message. Supports both Content-Length framed (spec-compliant)
/// and bare-JSON-per-line (some clients do this).
async fn read_mcp_message<R: AsyncBufReadExt + Unpin>(reader: &mut R) -> Result<Option<String>> {
    let mut header_line = String::new();

    loop {
        header_line.clear();
        let n = reader.read_line(&mut header_line).await?;
        if n == 0 {
            return Ok(None); // EOF
        }

        let trimmed = header_line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // If the line starts with '{', it's bare JSON (no Content-Length framing)
        if trimmed.starts_with('{') {
            return Ok(Some(trimmed.to_string()));
        }

        // Otherwise expect "Content-Length: NNN"
        if let Some(len_str) = trimmed.strip_prefix("Content-Length:") {
            let content_length: usize = len_str.trim().parse().unwrap_or(0);
            if content_length == 0 {
                continue;
            }

            // Read past the blank line separating headers from body
            let mut blank = String::new();
            loop {
                blank.clear();
                reader.read_line(&mut blank).await?;
                if blank.trim().is_empty() {
                    break;
                }
                // Skip any other headers (Content-Type, etc.)
            }

            // Read exactly content_length bytes
            let mut body = vec![0u8; content_length];
            reader.read_exact(&mut body).await?;
            let body_str = String::from_utf8_lossy(&body).to_string();
            return Ok(Some(body_str));
        }

        // Unknown header line, skip
    }
}

async fn write_mcp_message<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    response: &protocol::JsonRpcResponse,
) -> Result<()> {
    let body = serde_json::to_string(response)?;
    eprintln!("AESP SEND: {}", &body[..body.len().min(300)]);
    writer.write_all(body.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    Ok(())
}

fn handle_request(
    request: &protocol::JsonRpcRequest,
    storage: &Storage,
    config: &AespConfig,
    project_root: &PathBuf,
    server_info: &protocol::ServerInfo,
) -> Result<protocol::JsonRpcResponse> {
    let result = match request.method.as_str() {
        "initialize" => {
            let capabilities = protocol::ServerCapabilities {
                tools: Some(protocol::ToolsCapability {
                    list_changed: Some(false),
                }),
            };
            let init_result = serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": capabilities,
                "serverInfo": {
                    "name": server_info.name,
                    "version": server_info.version,
                },
                "instructions": "IMPORTANT: This server provides the aesp_start_task tool which MUST be called before reading any project files. It returns relevant code context, active constraints, and past decisions for any coding task — much faster than reading files manually. Always call aesp_start_task first with a description of the task."
            });
            Ok(init_result)
        }
        "notifications/initialized" => {
            return Ok(protocol::JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.clone(),
                result: Some(serde_json::json!({})),
                error: None,
            });
        }
        "tools/list" => {
            let tool_list = tools::get_tool_definitions();
            let result = serde_json::json!({ "tools": tool_list });
            eprintln!("AESP TOOLS/LIST: {} tools, first schema: {}",
                tool_list.len(),
                tool_list.first()
                    .and_then(|t| t.get("inputSchema"))
                    .map(|s| serde_json::to_string(s).unwrap_or_default())
                    .unwrap_or_else(|| "MISSING".to_string())
            );
            Ok(result)
        }
        "tools/call" => {
            let params = request.params.as_ref();
            let tool_name = params
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let arguments = params
                .and_then(|p| p.get("arguments"))
                .cloned()
                .unwrap_or(serde_json::json!({}));

            eprintln!("AESP TOOL CALL: {} args={}", tool_name,
                serde_json::to_string(&arguments).unwrap_or_default().chars().take(500).collect::<String>());

            handlers::handle_tool_call(tool_name, &arguments, storage, config, project_root)
        }
        _ => {
            eprintln!("AESP: unknown method: {}", request.method);
            Err(anyhow::anyhow!("Unknown method: {}", request.method))
        }
    };

    match result {
        Ok(value) => Ok(protocol::JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id.clone(),
            result: Some(value),
            error: None,
        }),
        Err(e) => {
            eprintln!("AESP: tool error: {}", e);
            Ok(protocol::JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.clone(),
                result: None,
                error: Some(protocol::JsonRpcError {
                    code: -32603,
                    message: e.to_string(),
                    data: None,
                }),
            })
        }
    }
}
