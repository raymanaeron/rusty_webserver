// Example tunnel client demonstrating WebSocket-based HTTP tunneling
// This client connects to the tunnel server and forwards HTTP requests to a local server

use httpserver_tunnel::protocol::{TunnelMessage, TunnelProtocol};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::collections::HashMap;
use tracing::{info, error, debug};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Configuration
    let tunnel_server_url = "ws://localhost:8080/connect";
    let auth_token = "test-token-123";
    let requested_subdomain = Some("myapp");
    let local_server_port = 3000;

    info!("Starting tunnel client...");
    info!("Connecting to tunnel server: {}", tunnel_server_url);
    info!("Local server port: {}", local_server_port);

    // Connect to tunnel server
    let (ws_stream, _) = connect_async(tunnel_server_url).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    info!("Connected to tunnel server, authenticating...");

    // Send authentication
    let auth_message = TunnelProtocol::create_auth_message(auth_token, requested_subdomain);
    let auth_data = TunnelProtocol::serialize_message(&auth_message)?;
    ws_sender.send(Message::Binary(auth_data)).await?;

    // Wait for authentication response
    if let Some(msg) = ws_receiver.next().await {
        let msg = msg?;
        if let Message::Binary(data) = msg {
            let response = TunnelProtocol::deserialize_message(&data)?;
            match response {
                TunnelMessage::AuthResponse { success, assigned_subdomain, error } => {
                    if success {
                        info!("Authentication successful! Assigned subdomain: {:?}", assigned_subdomain);
                    } else {
                        error!("Authentication failed: {:?}", error);
                        return Ok(());
                    }
                }
                _ => {
                    error!("Unexpected response to authentication");
                    return Ok(());
                }
            }
        }
    }

    info!("Tunnel client ready, waiting for HTTP requests...");    // Start heartbeat task
    let mut ws_sender_heartbeat = ws_sender.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            let ping = TunnelProtocol::create_ping_message();
            if let Ok(data) = TunnelProtocol::serialize_message(&ping) {
                if let Err(e) = ws_sender_heartbeat.send(Message::Binary(data)).await {
                    error!("Failed to send heartbeat: {}", e);
                    break;
                }
            }
        }
    });

    // Handle incoming tunnel messages
    while let Some(msg) = ws_receiver.next().await {
        let msg = msg?;
        if let Message::Binary(data) = msg {
            if let Ok(tunnel_msg) = TunnelProtocol::deserialize_message(&data) {
                match tunnel_msg {
                    TunnelMessage::HttpRequest { id, method, path, headers, body, client_ip } => {
                        debug!("Received HTTP request {}: {} {}", id, method, path);
                        
                        // Forward request to local server
                        let response = forward_to_local_server(
                            &method, &path, &headers, body, local_server_port
                        ).await;

                        // Send response back through tunnel
                        let response_msg = TunnelMessage::HttpResponse {
                            id,
                            status: response.status,
                            headers: response.headers,
                            body: response.body,
                        };

                        if let Ok(response_data) = TunnelProtocol::serialize_message(&response_msg) {
                            if let Err(e) = ws_sender.send(Message::Binary(response_data)).await {
                                error!("Failed to send response: {}", e);
                            }
                        }
                    }
                    TunnelMessage::Ping { timestamp } => {
                        // Respond to ping
                        let pong = TunnelMessage::Pong { timestamp };
                        if let Ok(data) = TunnelProtocol::serialize_message(&pong) {
                            let _ = ws_sender.send(Message::Binary(data)).await;
                        }
                    }
                    TunnelMessage::SslConnect { id, initial_data } => {
                        debug!("Received SSL connect request {}", id);
                        // TODO: Handle SSL passthrough connections
                    }
                    _ => {
                        debug!("Unhandled tunnel message type");
                    }
                }
            }
        }
    }

    info!("Tunnel client disconnected");
    Ok(())
}

#[derive(Debug)]
struct HttpResponse {
    status: u16,
    headers: HashMap<String, String>,
    body: Option<Vec<u8>>,
}

async fn forward_to_local_server(
    method: &str,
    path: &str,
    headers: &HashMap<String, String>,
    body: Option<Vec<u8>>,
    port: u16,
) -> HttpResponse {
    // Connect to local server
    let stream = match TcpStream::connect(format!("127.0.0.1:{}", port)).await {
        Ok(stream) => stream,
        Err(e) => {
            error!("Failed to connect to local server: {}", e);
            return HttpResponse {
                status: 502,
                headers: HashMap::new(),
                body: Some(b"Bad Gateway: Could not connect to local server".to_vec()),
            };
        }
    };

    let (mut reader, mut writer) = stream.into_split();

    // Build HTTP request
    let mut request = format!("{} {} HTTP/1.1\r\n", method, path);
    
    // Add headers
    for (name, value) in headers {
        request.push_str(&format!("{}: {}\r\n", name, value));
    }

    // Add content-length if body is present
    if let Some(ref body_data) = body {
        request.push_str(&format!("Content-Length: {}\r\n", body_data.len()));
    }

    request.push_str("\r\n");

    // Write request
    if let Err(e) = writer.write_all(request.as_bytes()).await {
        error!("Failed to write request: {}", e);
        return HttpResponse {
            status: 502,
            headers: HashMap::new(),
            body: Some(b"Bad Gateway: Failed to send request".to_vec()),
        };
    }

    // Write body if present
    if let Some(body_data) = body {
        if let Err(e) = writer.write_all(&body_data).await {
            error!("Failed to write request body: {}", e);
            return HttpResponse {
                status: 502,
                headers: HashMap::new(),
                body: Some(b"Bad Gateway: Failed to send request body".to_vec()),
            };
        }
    }

    // Read response
    let mut response_data = Vec::new();
    match reader.read_to_end(&mut response_data).await {
        Ok(_) => {
            parse_http_response(&response_data)
        }
        Err(e) => {
            error!("Failed to read response: {}", e);
            HttpResponse {
                status: 502,
                headers: HashMap::new(),
                body: Some(b"Bad Gateway: Failed to read response".to_vec()),
            }
        }
    }
}

fn parse_http_response(data: &[u8]) -> HttpResponse {
    let response_str = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => {
            return HttpResponse {
                status: 502,
                headers: HashMap::new(),
                body: Some(b"Bad Gateway: Invalid response encoding".to_vec()),
            };
        }
    };

    let mut lines = response_str.lines();
    
    // Parse status line
    let status_line = match lines.next() {
        Some(line) => line,
        None => {
            return HttpResponse {
                status: 502,
                headers: HashMap::new(),
                body: Some(b"Bad Gateway: Empty response".to_vec()),
            };
        }
    };

    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(502);

    // Parse headers
    let mut headers = HashMap::new();
    let mut body_start = 0;

    for (i, line) in lines.enumerate() {
        if line.is_empty() {
            body_start = response_str.find("\r\n\r\n")
                .or_else(|| response_str.find("\n\n"))
                .map(|pos| pos + 4)
                .unwrap_or(data.len());
            break;
        }

        if let Some(colon_pos) = line.find(':') {
            let name = line[..colon_pos].trim().to_string();
            let value = line[colon_pos + 1..].trim().to_string();
            headers.insert(name, value);
        }
    }

    // Extract body
    let body = if body_start < data.len() {
        Some(data[body_start..].to_vec())
    } else {
        None
    };

    HttpResponse {
        status,
        headers,
        body,
    }
}
