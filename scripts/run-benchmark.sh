#!/bin/bash
#
# DDL Distributed Benchmark Runner
#
# This script manages a cluster of DDL nodes and runs benchmarks.
#
# Usage:
#   ./scripts/run-benchmark.sh [OPTIONS]
#
# Options:
#   --nodes <NUM>        Number of nodes in cluster (default: 3)
#   --duration <SEC>     Benchmark duration in seconds (default: 60)
#   --workload <TYPE>    Workload type: setup, topic, consensus, load, all (default: all)
#   --output <FILE>      Output file for results (default: stdout)
#   --keep-nodes         Keep nodes running after benchmark (for debugging)
#   --build              Build binaries before running
#   --help               Show this help message
#
# Examples:
#   # Run full benchmark with 3 nodes
#   ./scripts/run-benchmark.sh --nodes 3 --duration 60
#
#   # Build and run
#   ./scripts/run-benchmark.sh --build --nodes 5
#
#   # Run specific workload
#   ./scripts/run-benchmark.sh --workload topic --nodes 3
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
NUM_NODES=3
DURATION=60
WORKLOAD="all"
OUTPUT=""
KEEP_NODES=false
BUILD=false
BASE_PORT=7000

# Parse arguments
while [[ $# -gt 0 ]]; do
	case $1 in
	--nodes)
		NUM_NODES="$2"
		shift 2
		;;
	--duration)
		DURATION="$2"
		shift 2
		;;
	--workload)
		WORKLOAD="$2"
		shift 2
		;;
	--output)
		OUTPUT="$2"
		shift 2
		;;
	--keep-nodes)
		KEEP_NODES=true
		shift
		;;
	--build)
		BUILD=true
		shift
		;;
	--help)
		cat "$0" | grep '^#' | sed 's/^# //'
		exit 0
		;;
	*)
		echo -e "${RED}Unknown option: $1${NC}"
		exit 1
		;;
	esac
done

echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}DDL Distributed Benchmark${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""

# Build binaries if requested
if [ "$BUILD" = true ]; then
	echo -e "${YELLOW}Building binaries...${NC}"
	cargo build --release --bin ddl-node --bin ddl-benchmark 2>&1 | grep -v Compiling | grep -v Finished | grep -v Running
	if [ ${PIPESTATUS[0]} -ne 0 ]; then
		echo -e "${RED}Build failed!${NC}"
		exit 1
	fi
	echo -e "${GREEN}Build complete${NC}"
	echo ""
fi

# Store node PIDs
declare -a NODE_PIDS
declare -a NODE_PORTS

# Cleanup function
cleanup() {
	if [ "$KEEP_NODES" = false ]; then
		echo ""
		echo -e "${YELLOW}Stopping nodes...${NC}"
		for pid in "${NODE_PIDS[@]}"; do
			if kill -0 "$pid" 2>/dev/null; then
				kill "$pid" 2>/dev/null || true
				wait "$pid" 2>/dev/null || true
			fi
		done
		echo -e "${GREEN}All nodes stopped${NC}"
	else
		echo ""
		echo -e "${YELLOW}Keeping nodes running. PIDs: ${NODE_PIDS[*]}${NC}"
	fi
}

# Set trap for cleanup
trap cleanup EXIT

# Start node function
start_node() {
	local ID=$1
	local PORT=$2
	local IS_BOOTSTRAP=$3
	local PEERS=$4
	local LOG_FILE="/tmp/ddl-node-${ID}.log"

	echo -e "${YELLOW}Starting node ${ID} on port ${PORT}...${NC}"

	local CMD="cargo run --release --bin ddl-node -- --id ${ID} --port ${PORT}"

	if [ "$IS_BOOTSTRAP" = true ]; then
		CMD="$CMD --bootstrap"
	fi

	if [ -n "$PEERS" ]; then
		CMD="$CMD --peers ${PEERS}"
	fi

	CMD="$CMD --host 127.0.0.1"

	if [ "$OUTPUT" != "" ]; then
		CMD="$CMD --data-dir /tmp/ddl-data-${ID}"
	fi

	$CMD >"$LOG_FILE" 2>&1 &
	local PID=$!

	NODE_PIDS+=($PID)
	NODE_PORTS+=($PORT)

	echo "  PID: $PID"
	echo "  Log: $LOG_FILE"

	# Wait for node to start
	sleep 1

	# Check if process is still running
	if ! kill -0 $PID 2>/dev/null; then
		echo -e "${RED}Node ${ID} failed to start!${NC}"
		echo "Check log file: $LOG_FILE"
		exit 1
	fi

	echo -e "${GREEN}Node ${ID} started successfully${NC}"
}

# Wait for cluster to be ready
wait_for_cluster() {
	echo ""
	echo -e "${YELLOW}Waiting for cluster to be ready...${NC}"

	local MAX_WAIT=30
	local COUNT=0

	# Give nodes time to start
	sleep 3

	# Wait for leader election
	echo "  Waiting for leader election..."
	sleep 2

	echo -e "${GREEN}Cluster is ready${NC}"
	echo ""
}

echo -e "${BLUE}Configuration:${NC}"
echo "  Nodes: $NUM_NODES"
echo "  Duration: ${DURATION}s"
echo "  Workload: $WORKLOAD"
echo "  Base port: $BASE_PORT"
echo ""

# Construct peer list for non-bootstrap nodes
PEERS=""
if [ $NUM_NODES -gt 1 ]; then
	for i in $(seq 2 $NUM_NODES); do
		port=$((BASE_PORT + i - 1))
		if [ -n "$PEERS" ]; then
			PEERS="${PEERS},"
		fi
		PEERS="${PEERS}127.0.0.1:${BASE_PORT}"
	done
fi

# Start nodes
echo -e "${BLUE}Starting cluster nodes:${NC}"
for i in $(seq 1 $NUM_NODES); do
	PORT=$((BASE_PORT + i - 1))
	IS_BOOTSTRAP=false
	if [ $i -eq 1 ]; then
		IS_BOOTSTRAP=true
	fi

	start_node $i $PORT $IS_BOOTSTRAP "$PEERS"
done

# Wait for cluster to be ready
wait_for_cluster

# Run benchmarks
echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}Running Benchmarks${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""

# Build benchmark specification
BENCHMARK_SPEC="localhost:[${BASE_PORT}-$((BASE_PORT + NUM_NODES - 1))]"

# Run the appropriate benchmark
BENCHMARK_CMD="cargo test --release --test distributed_benchmark --features integration-tests -- --nocapture"

case $WORKLOAD in
setup)
	echo -e "${YELLOW}Running cluster setup benchmark...${NC}"
	$BENCHMARK_CMD benchmark_cluster_setup 2>&1 | tee /tmp/ddl-benchmark-output.txt
	;;
topic)
	echo -e "${YELLOW}Running topic operations benchmark...${NC}"
	$BENCHMARK_CMD benchmark_topic_operations 2>&1 | tee /tmp/ddl-benchmark-output.txt
	;;
consensus)
	echo -e "${YELLOW}Running consensus operations benchmark...${NC}"
	$BENCHMARK_CMD benchmark_consensus_operations 2>&1 | tee /tmp/ddl-benchmark-output.txt
	;;
load)
	echo -e "${YELLOW}Running concurrent load benchmark...${NC}"
	$BENCHMARK_CMD benchmark_concurrent_load 2>&1 | tee /tmp/ddl-benchmark-output.txt
	;;
all | *)
	echo -e "${YELLOW}Running full benchmark suite...${NC}"
	RUST_TEST_NODES="$BENCHMARK_SPEC" RUST_TEST_DURATION="$DURATION" \
		$BENCHMARK_CMD full_benchmark_suite 2>&1 | tee /tmp/ddl-benchmark-output.txt
	;;
esac

# Save results if output file specified
if [ -n "$OUTPUT" ]; then
	cp /tmp/ddl-benchmark-output.txt "$OUTPUT"
	echo ""
	echo -e "${GREEN}Results saved to: $OUTPUT${NC}"
fi

echo ""
echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}Benchmark Complete${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""

# Generate summary
if [ -f /tmp/ddl-benchmark-output.txt ]; then
	echo -e "${GREEN}Summary:${NC}"
	grep -E "(throughput|latency|Setup Time|Leader Election|Memory|Contention)" /tmp/ddl-benchmark-output.txt | head -20 || true
fi

# Cleanup will be called automatically via trap
