#!/bin/bash

# Test script for gRPC API using grpcurl
# Requires: grpcurl (brew install grpcurl)

BASE_URL="localhost:50051"

echo "=== Testing Spec Service gRPC API ==="
echo ""

# Check if grpcurl is installed
if ! command -v grpcurl &> /dev/null; then
    echo "grpcurl is not installed. Install with: brew install grpcurl"
    exit 1
fi

# List services
echo "1. Listing available services:"
grpcurl -plaintext $BASE_URL list
echo ""

# Create a spec
echo "2. Creating a new spec:"
SPEC_RESPONSE=$(grpcurl -plaintext -d '{
  "name": "grpc-test-spec",
  "content": "rules:\n  - pattern: \"^test$\"\n    action: allow",
  "description": "Test spec via gRPC"
}' $BASE_URL spec.SpecService/CreateSpec)

echo "$SPEC_RESPONSE"
SPEC_ID=$(echo "$SPEC_RESPONSE" | jq -r '.id')
echo ""

# Wait for projections
sleep 0.5

# Get the spec
echo "3. Getting the created spec:"
grpcurl -plaintext -d "{\"id\": \"$SPEC_ID\"}" $BASE_URL spec.SpecService/GetSpec | jq '.'
echo ""

# Update the spec
echo "4. Updating the spec:"
grpcurl -plaintext -d "{
  \"id\": \"$SPEC_ID\",
  \"content\": \"rules:\\n  - pattern: \\\"^test$\\\"\\n    action: allow\\n  - pattern: \\\"^prod$\\\"\\n    action: deny\",
  \"description\": \"Updated via gRPC\"
}" $BASE_URL spec.SpecService/UpdateSpec | jq '.'
echo ""

# List specs
echo "5. Listing all specs:"
grpcurl -plaintext -d '{"page_size": 10}' $BASE_URL spec.SpecService/ListSpecs | jq '.'
echo ""

# Publish the spec
echo "6. Publishing the spec:"
grpcurl -plaintext -d "{\"id\": \"$SPEC_ID\"}" $BASE_URL spec.SpecService/PublishSpec | jq '.'
echo ""

# Get spec history
echo "7. Getting spec history:"
grpcurl -plaintext -d "{\"id\": \"$SPEC_ID\"}" $BASE_URL spec.SpecService/GetSpecHistory | jq '.'
echo ""

echo "=== gRPC API test complete ===="