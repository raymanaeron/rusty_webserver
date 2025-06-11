#!/bin/bash

# Comprehensive test script for the HTTP server
echo "🧪 Running comprehensive tests for HTTP Server..."
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test results tracking
TOTAL_TESTS=0
PASSED_TESTS=0

# Function to run a test
run_test() {
    local test_name="$1"
    local test_command="$2"
    local expected_pattern="$3"
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    echo -n "Testing $test_name... "
    
    result=$(eval "$test_command" 2>&1)
    
    if echo "$result" | grep -q "$expected_pattern"; then
        echo -e "${GREEN}✓ PASSED${NC}"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        echo -e "${RED}✗ FAILED${NC}"
        echo "  Expected: $expected_pattern"
        echo "  Got: $result"
    fi
}

# Build and start server for testing
echo "🔨 Building server for testing..."
cargo build --release > /dev/null 2>&1

echo "🚀 Starting test server on port 9999..."
./target/release/httpserver --directory . --port 9999 > /dev/null 2>&1 &
SERVER_PID=$!

# Wait for server to start
sleep 2

echo "🧪 Running tests..."
echo ""

# Test 1: Basic connectivity
run_test "Basic connectivity" \
    "curl -s -o /dev/null -w '%{http_code}' http://localhost:9999/" \
    "200"

# Test 2: HTML MIME type
run_test "HTML MIME type" \
    "curl -s -I http://localhost:9999/ | grep 'content-type'" \
    "text/html"

# Test 3: CSS MIME type
run_test "CSS MIME type" \
    "curl -s -I http://localhost:9999/test.css | grep 'content-type'" \
    "text/css"

# Test 4: JavaScript MIME type
run_test "JavaScript MIME type" \
    "curl -s -I http://localhost:9999/test.js | grep 'content-type'" \
    "text/javascript"

# Test 5: JSON MIME type
run_test "JSON MIME type" \
    "curl -s -I http://localhost:9999/test.json | grep 'content-type'" \
    "application/json"

# Test 6: SVG MIME type
run_test "SVG MIME type" \
    "curl -s -I http://localhost:9999/test.svg | grep 'content-type'" \
    "image/svg+xml"

# Test 7: CORS headers
run_test "CORS headers" \
    "curl -s -I http://localhost:9999/ | grep 'access-control-allow-origin'" \
    "\\*"

# Test 8: Cache headers
run_test "Cache headers" \
    "curl -s -I http://localhost:9999/test.css | grep 'cache-control'" \
    "public, max-age=3600"

# Test 9: 404 handling (SPA fallback)
run_test "404/SPA fallback" \
    "curl -s -o /dev/null -w '%{http_code}' http://localhost:9999/nonexistent.html" \
    "200"

# Test 10: JSON content
run_test "JSON content parsing" \
    "curl -s http://localhost:9999/test.json | jq -r '.server'" \
    "Rust HTTP Server with Axum"

# Test 11: Command line help
run_test "Command line help" \
    "./target/release/httpserver --help" \
    "A simple cross-platform HTTP server"

# Test 12: Directory traversal protection
run_test "Directory traversal protection" \
    "curl -s -o /dev/null -w '%{http_code}' 'http://localhost:9999/../etc/passwd'" \
    "403\\|404"

echo ""
echo "🧪 Test Results:"
echo "  Total tests: $TOTAL_TESTS"
echo "  Passed: $PASSED_TESTS"
echo "  Failed: $((TOTAL_TESTS - PASSED_TESTS))"

if [ $PASSED_TESTS -eq $TOTAL_TESTS ]; then
    echo -e "${GREEN}🎉 All tests passed!${NC}"
    exit_code=0
else
    echo -e "${RED}❌ Some tests failed.${NC}"
    exit_code=1
fi

# Cleanup
echo ""
echo "🧹 Cleaning up..."
kill $SERVER_PID 2>/dev/null
wait $SERVER_PID 2>/dev/null

echo "✅ Test completed!"
exit $exit_code
