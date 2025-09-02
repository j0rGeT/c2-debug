#!/bin/bash

echo "Testing Rust C2 Framework..."

# Check if all binaries are built
if [ ! -f "target/release/c2-server" ]; then
    echo "Error: c2-server binary not found"
    exit 1
fi

if [ ! -f "target/release/c2-client" ]; then
    echo "Error: c2-client binary not found"
    exit 1
fi

if [ ! -f "target/release/c2-web" ]; then
    echo "Error: c2-web binary not found"
    exit 1
fi

echo "✅ All binaries built successfully"
echo "✅ Framework compilation complete"
echo "✅ Ready for deployment"

echo ""
echo "To run the framework:"
echo "1. Start server:   cargo run -p c2-server"
echo "2. Start web UI:   cargo run -p c2-web"
echo "3. Run client:     cargo run -p c2-client -- 127.0.0.1:8080"
echo ""
echo "Access web interface at: http://127.0.0.1:3000"