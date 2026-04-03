#!/bin/bash
# Test script for ddl-node client API
# This tests the TCP line protocol

set -e

echo "=== DDL Node Client API Test ==="
echo ""

# Start a bootstrap node in the background
echo "Starting bootstrap node on port 7000 with API on port 8080..."
cargo run --bin ddl-node -- --id 1 --port 7000 --bootstrap --api-port 8080 &
NODE_PID=$!

# Wait for node to start
sleep 5

# Function to send command and check response
test_command() {
	local cmd="$1"
	local expected="$2"
	local desc="$3"

	echo "Test: $desc"
	echo "  Command: $cmd"
	actual=$(echo "$cmd" | nc -q 1 localhost 8080 2>/dev/null | head -n 2 | tail -n 1)
	echo "  Expected: $expected"
	echo "  Actual:   $actual"

	if [[ "$actual" == *"$expected"* ]]; then
		echo "  ✓ PASS"
	else
		echo "  ✗ FAIL"
		kill $NODE_PID 2>/dev/null || true
		exit 1
	fi
	echo ""
}

# Test 1: Health check
test_command "PING" "PONG" "Health check"

# Test 2: Claim topic
test_command "CLAIM test.topic" "OK" "Claim topic ownership"

# Test 3: Get owner (should be node 1)
test_command "GET_OWNER test.topic" "OK 1" "Get topic owner after claim"

# Test 4: Try to claim same topic again (should fail or succeed depending on implementation)
echo "Test: Re-claim already owned topic"
echo "  Command: CLAIM test.topic"
actual=$(echo "CLAIM test.topic" | nc -q 1 localhost 8080 2>/dev/null | head -n 2 | tail -n 1)
echo "  Response: $actual"
echo "  (Implementation may allow re-claiming own topic)"
echo ""

# Test 5: Release topic
test_command "RELEASE test.topic" "OK" "Release topic ownership"

# Test 6: Get owner after release
test_command "GET_OWNER test.topic" "OK NONE" "Get topic owner after release"

# Test 7: Acquire lease
test_command "ACQUIRE_LEASE resource:vertex:1 60" "OK 1" "Acquire lease with 60s TTL"

# Test 8: Release lease
test_command "RELEASE_LEASE resource:vertex:1" "OK" "Release lease"

# Test 9: Invalid commands
test_command "CLAIM" "ERROR" "Error on missing argument"
test_command "INVALID_COMMAND" "ERROR" "Error on unknown command"

# Test 10: Help command
echo "Test: Help command"
help_output=$(echo "HELP" | nc -q 1 localhost 8080 2>/dev/null)
if [[ "$help_output" == *"CLAIM"* && "$help_output" == *"RELEASE"* && "$help_output" == *"PING"* ]]; then
	echo "  ✓ PASS - Help shows all commands"
else
	echo "  ✗ FAIL - Help incomplete"
	kill $NODE_PID 2>/dev/null || true
	exit 1
fi
echo ""

# Test 11: Graceful quit
echo "Test: Graceful disconnect"
echo "  Command: QUIT"
response=$(echo "QUIT" | nc -q 1 localhost 8080 2>/dev/null | head -n 1)
echo "  Response: $response"
if [[ "$response" == *"Bye"* ]]; then
	echo "  ✓ PASS"
else
	echo "  ✗ FAIL"
	kill $NODE_PID 2>/dev/null || true
	exit 1
fi
echo ""

# Cleanup
echo "Cleaning up..."
kill $NODE_PID 2>/dev/null || true
wait $NODE_PID 2>/dev/null || true

echo ""
echo "=== All Tests Passed ==="
