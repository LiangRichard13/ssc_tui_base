use super::protocol::McpServerConfig;
use super::transport::transport_for;
use super::transport::MessageTransport;
use crate::mcp::transport::HttpMessageTransport;
use crate::mcp::protocol::JsonRpcRequest;
use serde_json::Value;
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[test]
fn trait_object_compiles() {
    fn _assert_dyn_compatible(_t: Box<dyn MessageTransport>) {}
}

#[test]
fn stdio_config_picks_stdio_transport() {
    let cfg = McpServerConfig {
        transport: crate::mcp::McpTransport::Stdio,
        command: "echo".to_string(),
        args: vec![],
        env: Default::default(),
        url: None,
        headers: Default::default(),
        shared: true,
    };
    assert!(matches!(transport_for(&cfg), Ok(_)));
}

#[test]
fn http_config_picks_http_transport() {
    let cfg = McpServerConfig {
        transport: crate::mcp::McpTransport::Http,
        command: String::new(),
        args: vec![],
        env: Default::default(),
        url: Some("http://localhost:0/mcp".to_string()),
        headers: Default::default(),
        shared: true,
    };
    let t = transport_for(&cfg).expect("http transport should construct");
    let debug = format!("{:?}", t);
    assert!(debug.contains("Http"), "expected Http transport, got: {debug}");
}

/// Read an HTTP request from a stream. Returns (method, path, headers, body).
async fn read_http_request(
    stream: &mut tokio::net::TcpStream,
) -> (String, String, HashMap<String, String>, String) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    loop {
        let n = stream.read(&mut tmp).await.unwrap();
        buf.extend_from_slice(&tmp[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        if n == 0 {
            break;
        }
    }
    let raw = String::from_utf8_lossy(&buf).to_string();
    let mut lines = raw.split("\r\n");
    let request_line = lines.next().unwrap_or("").to_string();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let mut headers = HashMap::new();
    let mut content_length: usize = 0;
    for line in lines.by_ref() {
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim().to_ascii_lowercase();
            let v = v.trim().to_string();
            if k == "content-length" {
                content_length = v.parse().unwrap_or(0);
            }
            headers.insert(k, v);
        }
    }
    let mut body = String::new();
    if let Some(idx) = raw.find("\r\n\r\n") {
        body = raw[idx + 4..].to_string();
    }
    while body.len() < content_length {
        let n = stream.read(&mut tmp).await.unwrap();
        body.push_str(&String::from_utf8_lossy(&tmp[..n]));
    }
    (method, path, headers, body)
}

fn ok_json_response(id: u64, body: &str) -> String {
    let resp = format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{body}}}");
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        resp.len(),
        resp
    )
}

#[tokio::test]
async fn http_transport_round_trips_initialize_and_tools_list() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let url = format!("http://{addr}/mcp");

    // Server: simulate MCP responses
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let (_method, _path, headers, body) = read_http_request(&mut stream).await;
        assert_eq!(headers.get("x-api-key").map(String::as_str), Some("sk-test"));
        assert_eq!(headers.get("content-type").map(String::as_str), Some("application/json"));
        assert_eq!(headers.get("accept").map(String::as_str), Some("application/json, text/event-stream"));

        let req: Value = serde_json::from_str(&body).unwrap();
        let id = req.get("id").and_then(Value::as_u64).unwrap();

        // Send initialize response
        stream
            .write_all(
                ok_json_response(
                    id,
                    r#"{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"fake","version":"0.1.0"}}"#,
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        stream.flush().await.unwrap();

        // Read tools/list request
        let (_m2, _p2, _h2, body2) = read_http_request(&mut stream).await;
        let req2: Value = serde_json::from_str(&body2).unwrap();
        let id2 = req2.get("id").and_then(Value::as_u64).unwrap();

        // Send tools/list response
        stream
            .write_all(
                ok_json_response(
                    id2,
                    r#"{"tools":[{"name":"ping","description":"test tool","inputSchema":{"type":"object"}}]}"#,
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        stream.flush().await.unwrap();
    });

    // Client
    let mut hdrs = HashMap::new();
    hdrs.insert("X-API-Key".to_string(), "sk-test".to_string());
    let transport = HttpMessageTransport::new(url, hdrs);

    // Initialize
    let init_req = JsonRpcRequest::new(
        1,
        "initialize",
        Some(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "0.0.0"}
        })),
    );
    let init_value = transport
        .round_trip(serde_json::to_string(&init_req).unwrap() + "\n")
        .await
        .unwrap();
    assert_eq!(
        init_value.get("id").and_then(Value::as_u64),
        Some(1)
    );
    assert!(init_value.get("result").is_some());

    // tools/list
    let list_req = JsonRpcRequest::new(2, "tools/list", None);
    let list_value = transport
        .round_trip(serde_json::to_string(&list_req).unwrap() + "\n")
        .await
        .unwrap();
    let tools = list_value
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].get("name").and_then(Value::as_str), Some("ping"));

    server.await.unwrap();
}

#[tokio::test]
async fn http_transport_parses_sse_response() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let url = format!("http://{addr}/mcp");

    // Server: respond with SSE
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let (_m, _p, _h, body) = read_http_request(&mut stream).await;
        let req: Value = serde_json::from_str(&body).unwrap();
        let id = req.get("id").and_then(Value::as_u64).unwrap();
        let payload = serde_json::json!({"jsonrpc":"2.0","id":id,"result":{"ok":true}});
        let sse = format!("event: message\r\ndata: {}\r\n\r\n", payload);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
            sse.len(),
            sse
        );
        stream.write_all(response.as_bytes()).await.unwrap();
        stream.flush().await.unwrap();
    });

    let mut hdrs = HashMap::new();
    hdrs.insert("X-API-Key".to_string(), "sk-sse".to_string());
    let transport = HttpMessageTransport::new(url, hdrs);
    let req = JsonRpcRequest::new(7, "ping", None);
    let value = transport
        .round_trip(serde_json::to_string(&req).unwrap() + "\n")
        .await
        .unwrap();
    assert_eq!(value.get("id").and_then(Value::as_u64), Some(7));
    assert!(value.get("result").is_some());

    server.await.unwrap();
}
