// Simple WebSocket server for testing end-to-end functionality
use tokio_tungstenite::{accept_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(&addr).await?;
    println!("WebSocket test server listening on: {}", addr);

    while let Ok((stream, addr)) = listener.accept().await {
        tokio::spawn(handle_connection(stream, addr));
    }

    Ok(())
}

async fn handle_connection(raw_stream: TcpStream, addr: SocketAddr) {
    println!("New WebSocket connection from: {}", addr);
    
    match accept_async(raw_stream).await {
        Ok(ws_stream) => {
            let (mut ws_sender, mut ws_receiver) = ws_stream.split();
            
            // Echo server - send back any message received
            while let Some(msg) = ws_receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        println!("Received text: {}", text);
                        
                        // Handle health check ping
                        if text == "ping" {
                            if let Err(e) = ws_sender.send(Message::Text("pong".to_string())).await {
                                println!("Error sending pong: {}", e);
                                break;
                            }
                        } else {
                            // Echo the message back
                            let response = format!("Echo: {}", text);
                            if let Err(e) = ws_sender.send(Message::Text(response)).await {
                                println!("Error sending echo: {}", e);
                                break;
                            }
                        }
                    }
                    Ok(Message::Binary(data)) => {
                        println!("Received binary data: {} bytes", data.len());
                        if let Err(e) = ws_sender.send(Message::Binary(data)).await {
                            println!("Error echoing binary data: {}", e);
                            break;
                        }
                    }
                    Ok(Message::Ping(data)) => {
                        println!("Received ping");
                        if let Err(e) = ws_sender.send(Message::Pong(data)).await {
                            println!("Error sending pong: {}", e);
                            break;
                        }
                    }
                    Ok(Message::Pong(_)) => {
                        println!("Received pong");
                    }
                    Ok(Message::Close(_)) => {
                        println!("Connection closed");
                        break;
                    }
                    Err(e) => {
                        println!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        }
        Err(e) => {
            println!("Error during WebSocket handshake: {}", e);
        }
    }
    
    println!("Connection with {} ended", addr);
}
