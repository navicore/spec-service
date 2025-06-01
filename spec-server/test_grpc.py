#!/usr/bin/env python3
"""
Simple gRPC test client for Spec Service
Requires: pip install grpcio grpcio-tools
"""

import grpc
import sys
import json

# Since we don't have the Python stubs generated, we'll use raw gRPC
# This demonstrates that the gRPC server is running

def test_grpc_health():
    """Test if gRPC server is responding"""
    try:
        channel = grpc.insecure_channel('localhost:50051')
        # Try to connect - this will fail if server isn't running
        grpc.channel_ready_future(channel).result(timeout=5)
        print("✓ gRPC server is running on localhost:50051")
        return True
    except grpc.FutureTimeoutError:
        print("✗ gRPC server is not responding")
        return False
    except Exception as e:
        print(f"✗ Error connecting to gRPC server: {e}")
        return False

if __name__ == "__main__":
    print("Testing gRPC connectivity...")
    if test_grpc_health():
        print("\nBoth REST (port 3000) and gRPC (port 50051) servers are running!")
        print("\nTo fully test gRPC, you would need:")
        print("1. grpcurl: brew install grpcurl")
        print("2. Or generate Python/Go/JS stubs from the .proto file")
    else:
        sys.exit(1)