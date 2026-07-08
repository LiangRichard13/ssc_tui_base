use super::protocol::McpServerConfig;
use super::transport::{transport_for, MessageTransport};

#[test]
fn trait_object_compiles() {
    // Asserts the trait is dyn-compatible (no Self: Sized bound on the
    // public surface, no generic methods).
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
