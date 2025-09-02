# Rust C2 Framework

A modern, secure, and high-performance Command & Control framework built with Rust for defensive security purposes.

## Features

- **End-to-end Encryption**: AES-256-GCM encryption with HMAC authentication
- **WebSocket Communication**: Real-time bidirectional communication
- **Web Management Interface**: Modern web-based administration console
- **Cross-platform**: Works on Windows, Linux, and macOS
- **Modular Architecture**: Separated into server, client, and common modules
- **Secure by Default**: Built with security best practices

## Architecture

```
c2-framework/
├── common/          # Shared types and crypto utilities
├── server/          # C2 server implementation
├── client/          # Client agent implementation
└── web/             # Web management interface
```

## Quick Start

### Prerequisites

- Rust 1.60+ and Cargo
- OpenSSL development libraries

### Building

```bash
# Build all components
cargo build --release

# Build specific components
cargo build -p c2-server --release
cargo build -p c2-client --release
cargo build -p c2-web --release
```

### Running

1. **Start the Server**:
   ```bash
   cargo run -p c2-server
   ```

2. **Start the Web Interface**:
   ```bash
   cargo run -p c2-web
   ```

3. **Run the Client**:
   ```bash
   cargo run -p c2-client -- 127.0.0.1:8080
   ```

## Security Features

- **AES-256-GCM Encryption**: All communications are encrypted
- **HMAC Authentication**: Message integrity verification
- **Secure Key Generation**: Cryptographically secure random keys
- **No Hardcoded Secrets**: Keys are generated at runtime
- **Input Validation**: Comprehensive input sanitization

## API Endpoints

### Server (Port 8080)
- `GET /health` - Health check
- `GET /clients` - List connected clients
- `WS /ws` - WebSocket endpoint for client communication

### Web Interface (Port 3000)
- `GET /` - Management console
- `GET /health` - Health check
- `GET /clients` - API endpoint for client list

## Configuration

Environment variables for configuration:
- `RUST_LOG` - Log level (e.g., `info`, `debug`)
- `C2_SERVER_HOST` - Server bind address (default: 127.0.0.1:8080)
- `C2_WEB_HOST` - Web interface bind address (default: 127.0.0.1:3000)

## Development

### Testing

```bash
# Run all tests
cargo test

# Test specific module
cargo test -p c2-common
```

### Logging

Enable debug logging:
```bash
RUST_LOG=debug cargo run -p c2-server
```

## License

This project is for educational and defensive security purposes only.

## Disclaimer

This software is intended for legitimate security testing, research, and educational purposes only. Users are responsible for complying with all applicable laws and regulations.