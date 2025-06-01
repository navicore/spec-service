#!/bin/bash

echo "=== Starting Spec Service REST API Demo ==="
echo ""

# Clean up any existing database
rm -f spec_service.db demo_spec_service.db

# Use file database for demo (in-memory doesn't share between connections)
export DATABASE_URL="sqlite:demo_spec_service.db?mode=rwc"

# Start the server in the background
echo "Starting server..."
cargo run &
SERVER_PID=$!

# Wait for server to start
echo "Waiting for server to be ready..."
sleep 3

# Check if server is running
if ! kill -0 $SERVER_PID 2>/dev/null; then
    echo "Server failed to start!"
    exit 1
fi

# Run the test script
echo ""
echo "Running API tests..."
echo ""
./test-api.sh

# Clean up
echo ""
echo "Stopping server..."
kill $SERVER_PID
wait $SERVER_PID 2>/dev/null

echo ""
echo "=== Demo complete ==="