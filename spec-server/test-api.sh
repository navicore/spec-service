#!/bin/bash

# Test script for REST API

BASE_URL="http://localhost:3000"

echo "=== Testing Spec Service REST API ==="
echo ""

# Health check
echo "1. Health check:"
curl -s $BASE_URL/health
echo -e "\n"

# Create a spec
echo "2. Creating a new spec:"
SPEC_ID=$(curl -s -X POST $BASE_URL/specs \
  -H "Content-Type: application/json" \
  -d '{
    "name": "test-validation-rules",
    "content": "rules:\n  - pattern: \"^[A-Z][a-z]+$\"\n    description: \"Capitalized word\"\n  - pattern: \"[0-9]\"\n    description: \"Contains digit\"",
    "description": "Test validation rules for demo"
  }' | jq -r '.id')

echo "Created spec with ID: $SPEC_ID"
echo ""

# Wait for event processor to update projections
sleep 0.5

# Get the spec
echo "3. Fetching the created spec:"
curl -s $BASE_URL/specs/$SPEC_ID | jq '.'
echo ""

# List all specs
echo "4. Listing all draft specs:"
curl -s "$BASE_URL/specs?state=draft" | jq '.'
echo ""

# Update the spec
echo "5. Updating the spec:"
VERSION=$(curl -s -X PUT $BASE_URL/specs/$SPEC_ID \
  -H "Content-Type: application/json" \
  -d '{
    "content": "rules:\n  - pattern: \"^[A-Z][a-z]+$\"\n    description: \"Capitalized word\"\n  - pattern: \"[0-9]\"\n    description: \"Contains digit\"\n  - pattern: \".{3,}\"\n    description: \"Minimum 3 characters\"",
    "description": "Updated with minimum length rule"
  }' | jq -r '.version')

echo "Updated to version: $VERSION"
echo ""

# Publish the spec
echo "6. Publishing the spec:"
curl -s -X POST $BASE_URL/specs/$SPEC_ID/publish \
  -H "Content-Type: application/json" \
  -d '{"version": 2}'
echo "Published successfully"
echo ""

# List published specs
echo "7. Listing all published specs:"
curl -s "$BASE_URL/specs?state=published" | jq '.'
echo ""

# Get version history
echo "8. Getting version 1 of the spec:"
curl -s $BASE_URL/specs/$SPEC_ID/versions/1 | jq '.'
echo ""

# Deprecate the spec
echo "9. Deprecating the spec:"
curl -s -X POST $BASE_URL/specs/$SPEC_ID/deprecate \
  -H "Content-Type: application/json" \
  -d '{"reason": "Replaced by test-validation-rules-v2"}'
echo "Deprecated successfully"
echo ""

# Final state
echo "10. Final state of the spec:"
curl -s $BASE_URL/specs/$SPEC_ID | jq '.state'
echo ""

echo "=== API test complete ==="