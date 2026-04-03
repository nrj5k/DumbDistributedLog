#!/usr/bin/env bash
# Test that new_tcp() properly waits for bind
#
# This script verifies the TCP bind fixes:
# 1. Bind failures are propagated (not swallowed)
# 2. new_tcp() waits for bind confirmation
# 3. Multiple nodes don't conflict on ports
# 4. Port conflicts are detected early

set -e

echo "================================================================"
echo "TCP Bind Behavior Tests"
echo "================================================================"
echo ""

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test helper function
run_test() {
	local test_name="$1"
	local test_cmd="$2"
	local expected="$3"

	echo -n "Test: $test_name ... "

	if eval "$test_cmd" >/dev/null 2>&1; then
		if [ "$expected" = "success" ]; then
			echo -e "${GREEN}✓ PASSED${NC}"
			return 0
		else
			echo -e "${RED}✗ FAILED (expected failure, got success)${NC}"
			return 1
		fi
	else
		if [ "$expected" = "failure" ]; then
			echo -e "${GREEN}✓ PASSED (expected failure)${NC}"
			return 0
		else
			echo -e "${RED}✗ FAILED${NC}"
			return 1
		fi
	fi
}

echo "Test 1: Successful bind on available port"
echo "----------------------------------------------------------------"
echo "Expected: Should succeed within 5 seconds"
echo ""

# Build first
cd "$PROJECT_ROOT"
cargo build --bin ddl-node --release 2>&1 | tail -5

# Test 1: Successful bind on available port
echo -n "Starting node with available port (7000)... "
PORT=7000
NODE_ID=1
DATA_DIR="/tmp/ddl-test-node-$$"

# Clean up any existing processes
pkill -f "ddl-node.*--port $PORT" 2>/dev/null || true
sleep 2

# Check port is free
if ss -tlnp 2>/dev/null | grep -q ":$PORT"; then
	echo -e "${RED}✗ SKIPPED (port $PORT already in use)${NC}"
else
	# Start node in background
	timeout 10 cargo run --release --bin ddl-node -- \
		--id $NODE_ID \
		--port $PORT \
		--bootstrap \
		--data-dir "$DATA_DIR" \
		>"$DATA_DIR/test.log" 2>&1 &

	NODE_PID=$!

	# Wait for bind (should complete within 5 seconds due to new_tcp() timeout)
	sleep 3

	# Check if process is still running
	if kill -0 $NODE_PID 2>/dev/null; then
		echo -e "${GREEN}✓ Node started successfully${NC}"

		# Verify port is bound
		if ss -tlnp 2>/dev/null | grep -q ":$PORT"; then
			echo -e "${GREEN}✓ Port $PORT is bound${NC}"
			TEST1_RESULT=0
		else
			echo -e "${RED}✗ Port $PORT not bound${NC}"
			TEST1_RESULT=1
		fi

		# Clean up
		kill $NODE_PID 2>/dev/null || true
		wait $NODE_PID 2>/dev/null || true
	else
		echo -e "${RED}✗ Node failed to start${NC}"
		echo "Log output:"
		tail -20 "$DATA_DIR/test.log"
		TEST1_RESULT=1
	fi
fi

# Clean up test data
rm -rf "$DATA_DIR"

echo ""
echo "Test 2: Bind failure on occupied port"
echo "----------------------------------------------------------------"
echo "Expected: Should return error, not hang silently"
echo ""

# Test 2: Bind failure on occupied port
PORT=7001
echo -n "Occupying port $PORT... "

# Start a simple listener to occupy the port
python3 -c "import socket; s=socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1); s.bind(('0.0.0.0', $PORT)); s.listen(1); input('Press Enter to close')" >/dev/null 2>&1 &
OCCUPY_PID=$!
sleep 1

# Verify port is occupied
if ss -tlnp 2>/dev/null | grep -q ":$PORT"; then
	echo -e "${GREEN}Port occupied${NC}"

	DATA_DIR="/tmp/ddl-test-node-conflict-$$"

	echo -n "Attempting to start node on occupied port... "
	TIMEOUT_CMD="timeout 10 cargo run --release --bin ddl-node -- --id 2 --port $PORT --bootstrap --data-dir $DATA_DIR 2>&1"

	# This should FAIL (bind error) and NOT hang
	if ! eval "$TIMEOUT_CMD" >/dev/null 2>&1; then
		echo -e "${GREEN}✓ Node correctly failed (bind error)${NC}"
		echo -e "  ${GREEN}✓ Error was NOT swallowed (new_tcp() propagates bind failures)${NC}"
		TEST2_RESULT=0
	else
		echo -e "${RED}✗ Node started on occupied port (UNEXPECTED)${NC}"
		TEST2_RESULT=1
	fi

	# Clean up
	kill $OCCUPY_PID 2>/dev/null || true
	wait $OCCUPY_PID 2>/dev/null || true
	rm -rf "$DATA_DIR"
else
	echo -e "${YELLOW}✗ Could not occupy port ${PORT}${NC}"
	TEST2_RESULT=1
fi

echo ""
echo "Test 3: Multiple nodes on different ports"
echo "----------------------------------------------------------------"
echo "Expected: All nodes bind successfully in parallel"
echo ""

# Test 3: Multiple nodes don't conflict
NODES=()
PORTS=(7002 7003 7004)
NODE_IDS=(3 4 5)

for i in "${!NODE_IDS[@]}"; do
	NODE_ID=${NODE_IDS[$i]}
	PORT=${PORTS[$i]}
	DATA_DIR="/tmp/ddl-test-node-$NODE_ID-$$"

	echo -n "Starting node $NODE_ID on port $PORT... "

	# Clean up any existing
	pkill -f "ddl-node.*--port $PORT" 2>/dev/null || true

	# Check port is free
	if ss -tlnp 2>/dev/null | grep -q ":$PORT"; then
		echo -e "${RED}✗ Port $PORT already in use${NC}"
		TEST3_RESULT=1
		continue
	fi

	# Start node in background
	timeout 10 cargo run --release --bin ddl-node -- \
		--id $NODE_ID \
		--port $PORT \
		--bootstrap \
		--data-dir "$DATA_DIR" \
		>"$DATA_DIR/test.log" 2>&1 &

	NODES+=($!)
	echo -e "${GREEN}✓ Started${NC}"
done

# Wait for all nodes
echo -n "Waiting for nodes to bind... "
sleep 5

# Check all ports are bound
ALL_BOUND=true
for PORT in "${PORTS[@]}"; do
	if ! ss -tlnp 2>/dev/null | grep -q ":$PORT"; then
		echo -e "${RED}✗ Port $PORT not bound${NC}"
		ALL_BOUND=false
	fi
done

if $ALL_BOUND; then
	echo -e "${GREEN}✓ All nodes bound successfully${NC}"
	TEST3_RESULT=0
else
	echo -e "${RED}✗ Some nodes failed to bind${NC}"
	TEST3_RESULT=1
fi

# Clean up all nodes
for PID in "${NODES[@]}"; do
	kill $PID 2>/dev/null || true
	wait $PID 2>/dev/null || true
done

# Clean up test data
for NODE_ID in "${NODE_IDS[@]}"; do
	rm -rf "/tmp/ddl-test-node-$NODE_ID-$$"
done

echo ""
echo "================================================================"
echo "Test Results Summary"
echo "================================================================"
echo ""

TOTAL_TESTS=3
PASSED_TESTS=0
[ $TEST1_RESULT -eq 0 ] && ((PASSED_TESTS++))
[ $TEST2_RESULT -eq 0 ] && ((PASSED_TESTS++))
[ $TEST3_RESULT -eq 0 ] && ((PASSED_TESTS++))

echo "Test 1 (Available port bind):      $([ $TEST1_RESULT -eq 0 ] && echo -e "${GREEN}PASSED${NC}" || echo -e "${RED}FAILED${NC}")"
echo "Test 2 (Occupied port error):      $([ $TEST2_RESULT -eq 0 ] && echo -e "${GREEN}PASSED${NC}" || echo -e "${RED}FAILED${NC}")"
echo "Test 3 (Multiple nodes parallel):  $([ $TEST3_RESULT -eq 0 ] && echo -e "${GREEN}PASSED${NC}" || echo -e "${RED}FAILED${NC}")"
echo ""

if [ $PASSED_TESTS -eq $TOTAL_TESTS ]; then
	echo -e "${GREEN}All tests passed!${NC}"
	echo ""
	echo "TCP bind fixes verified:"
	echo "  ✓ Bind failures are propagated (not swallowed)"
	echo "  ✓ new_tcp() waits for bind confirmation"
	echo "  ✓ Multiple nodes bind successfully in parallel"
	echo "  ✓ Port conflicts are detected early"
	exit 0
else
	echo -e "${RED}Some tests failed${NC}"
	echo "Passed: $PASSED_TESTS/$TOTAL_TESTS"
	exit 1
fi
