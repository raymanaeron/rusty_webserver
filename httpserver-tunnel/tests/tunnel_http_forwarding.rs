// End-to-end integration test for tunnel HTTP forwarding
// Tests the complete flow: public request -> tunnel server -> tunnel client -> local server -> response

use httpserver_tunnel::{TunnelServer, config::TunnelServerConfig};
use httpserver_tunnel::protocol::{TunnelMessage, TunnelProtocol};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, debug};

#[tokio::test]
#[ignore] // Requires complex setup with multiple services
async fn test_tunnel_http_forwarding() {
    // Initialize tracing for test debugging
    let _ = tracing_subscriber::fmt::try_init();

    // Create tunnel server configuration
    let config = create_test_tunnel_config();
    
    // Start tunnel server
    let tunnel_server = TunnelServer::new(config.clone()).expect("Failed to create tunnel server");
    
    // Start server in background
    let server_handle = tokio::spawn(async move {
        tunnel_server.start().await
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Start mock local server that the tunnel client will forward to
    let local_server_port = 3001;
    let local_server_handle = start_mock_local_server(local_server_port).await;

    // Start tunnel client
    let client_handle = start_test_tunnel_client(local_server_port).await;

    // Give everything time to initialize
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

    // Test HTTP request forwarding
    let test_response = test_http_request_through_tunnel().await;
    
    // Verify the response
    assert!(test_response.is_ok(), "HTTP request through tunnel failed: {:?}", test_response);
    
    // Cleanup
    server_handle.abort();
    local_server_handle.abort();
    client_handle.abort();
    
    info!("Tunnel HTTP forwarding test completed successfully");
}

fn create_test_tunnel_config() -> TunnelServerConfig {
    TunnelServerConfig {
        enabled: true,
        tunnel_port: 8080,
        public_port: 8081,
        public_https_port: 8443,
        base_domain: "tunnel.test".to_string(),
        max_tunnels: 10,
        subdomain_strategy: httpserver_tunnel::config::SubdomainStrategy::Random,
        auth: httpserver_tunnel::config::TunnelServerAuthConfig {
            required: false,
            api_keys: vec!["test-token".to_string()],
            tokens: vec![],
        },
        rate_limiting: httpserver_tunnel::config::TunnelRateLimitConfig {
            enabled: false,
            requests_per_minute: 100,
            bandwidth_mbps: 10.0,
        },
        ssl: httpserver_tunnel::config::TunnelServerSslConfig {
            enabled: false,
            cert_file: None,
            key_file: None,
        },
        network: httpserver_tunnel::config::TunnelServerNetworkConfig {
            bind_address: "127.0.0.1".to_string(),
            public_bind_address: "127.0.0.1".to_string(),
            max_connections_per_ip: 10,
        },
    }
}

async fn start_mock_local_server(port: u16) -> tokio::task::JoinHandle<()> {
    use axum::{routing::get, Router, response::Html};
    use axum::http::StatusCode;

    let app = Router::new()
        .route("/", get(|| async { Html("<h1>Hello from local server!</h1>") }))
        .route("/test", get(|| async { 
            (StatusCode::OK, "Test response from local server") 
        }))
        .route("/health", get(|| async { 
            (StatusCode::OK, "Local server is healthy") 
        }));

    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
            .await
            .expect("Failed to bind local server");
        
        info!("Mock local server started on port {}", port);
        
        axum::serve(listener, app)
            .await
            .expect("Local server failed");
    })
}

async fn start_test_tunnel_client(local_port: u16) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let tunnel_url = "ws://127.0.0.1:8080/connect";
        
        // Connect to tunnel server
        let (ws_stream, _) = connect_async(tunnel_url).await
            .expect("Failed to connect to tunnel server");
        
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        
        // Authenticate
        let auth_msg = TunnelProtocol::create_auth_message("test-token", Some("testapp"));
        let auth_data = TunnelProtocol::serialize_message(&auth_msg)
            .expect("Failed to serialize auth message");
        
        ws_sender.send(Message::Binary(auth_data)).await
            .expect("Failed to send auth message");
        
        // Wait for auth response
        if let Some(msg) = ws_receiver.next().await {
            let msg = msg.expect("WebSocket error");
            if let Message::Binary(data) = msg {
                let response = TunnelProtocol::deserialize_message(&data)
                    .expect("Failed to deserialize auth response");
                
                match response {
                    TunnelMessage::AuthResponse { success, assigned_subdomain, .. } => {
                        if success {
                            info!("Test tunnel client authenticated with subdomain: {:?}", assigned_subdomain);
                        } else {
                            panic!("Authentication failed");
                        }
                    }
                    _ => panic!("Unexpected auth response"),
                }
            }
        }
        
        // Handle incoming requests
        while let Some(msg) = ws_receiver.next().await {
            let msg = msg.expect("WebSocket error");
            if let Message::Binary(data) = msg {
                if let Ok(tunnel_msg) = TunnelProtocol::deserialize_message(&data) {
                    match tunnel_msg {
                        TunnelMessage::HttpRequest { id, method, path, headers, body, .. } => {
                            debug!("Tunnel client received HTTP request: {} {}", method, path);
                            
                            // Forward to local server
                            let response = forward_to_local_server(&method, &path, &headers, body, local_port).await;
                            
                            // Send response back
                            let response_msg = TunnelMessage::HttpResponse {
                                id,
                                status: response.status,
                                headers: response.headers,
                                body: response.body,
                            };
                            
                            if let Ok(response_data) = TunnelProtocol::serialize_message(&response_msg) {
                                let _ = ws_sender.send(Message::Binary(response_data)).await;
                            }
                        }
                        TunnelMessage::Ping { timestamp } => {
                            let pong = TunnelMessage::Pong { timestamp };
                            if let Ok(data) = TunnelProtocol::serialize_message(&pong) {
                                let _ = ws_sender.send(Message::Binary(data)).await;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    })
}

#[derive(Debug)]
struct TestHttpResponse {
    status: u16,
    headers: HashMap<String, String>,
    body: Option<Vec<u8>>,
}

async fn forward_to_local_server(
    method: &str,
    path: &str,
    _headers: &HashMap<String, String>,
    _body: Option<Vec<u8>>,
    port: u16,
) -> TestHttpResponse {
    // Simple HTTP client for testing
    let url = format!("http://127.0.0.1:{}{}", port, path);
    
    match reqwest::get(&url).await {
        Ok(response) => {
            let status = response.status().as_u16();
            let headers = response.headers()
                .iter()
                .map(|(name, value)| (name.to_string(), value.to_str().unwrap_or("").to_string()))
                .collect();
            
            match response.bytes().await {
                Ok(body) => TestHttpResponse {
                    status,
                    headers,
                    body: Some(body.to_vec()),
                },
                Err(_) => TestHttpResponse {
                    status,
                    headers,
                    body: None,
                },
            }
        }
        Err(_) => TestHttpResponse {
            status: 502,
            headers: HashMap::new(),
            body: Some(b"Bad Gateway".to_vec()),
        },
    }
}

async fn test_http_request_through_tunnel() -> Result<(), Box<dyn std::error::Error>> {
    // Make HTTP request to the public tunnel endpoint
    let client = reqwest::Client::new();
    
    // Request should go to testapp.tunnel.test:8081/test
    let response = client
        .get("http://127.0.0.1:8081/test")
        .header("Host", "testapp.tunnel.test")
        .send()
        .await?;
    
    let status = response.status();
    let body = response.text().await?;
    
    info!("Tunnel response status: {}", status);
    info!("Tunnel response body: {}", body);
    
    // Verify response
    assert_eq!(status, 200);
    assert!(body.contains("Test response from local server"));
    
    Ok(())
}
