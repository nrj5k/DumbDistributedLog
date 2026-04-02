#!/bin/bash
#
# Start a DDL cluster
#
# Usage: ./scripts/start-cluster.sh <num_nodes> [base_port]
#

set -e

NUM_NODES=${1:-3}
BASE_PORT=${2:-7000}

echo "Starting DDL cluster with $NUM_NODES nodes (base port: $BASE_PORT)"

# Build the binary
echo "Building ddl-node binary..."
cargo build --release --bin ddl-node 2>&1 | grep -v Compiling | grep -v Finished || true

# Store PIDs in a file
PID_FILE="/tmp/ddl-cluster-pids.txt"
rm -f "$PID_FILE"

# Start bootstrap node (first node)
PORT=$((BASE_PORT))
echo "Starting bootstrap node on port $PORT..."
cargo run --release --bin ddl-node -- \
	--id 1 \
	--port $PORT \
	--bootstrap \
	--host 127.0.0.1 \
	--data-dir /tmp/ddl-data-1 \
	>/tmp/ddl-node-1.log 2>&1 &

echo $! >>"$PID_FILE"
sleep 2

# Start joining nodes
for i in $(seq 2 $NUM_NODES); do
	PORT=$((BASE_PORT + i - 1))
	PEER="127.0.0.1:$BASE_PORT"

	echo "Starting node $i on port $PORT..."
	cargo run --release --bin ddl-node -- \
		--id $i \
		--port $PORT \
		--peers "$PEER" \
		--host 127.0.0.1 \
		--data-dir /tmp/ddl-data-$i \
		>/tmp/ddl-node-$i.log 2>&1 &

	echo $! >>"$PID_FILE"
	sleep 1
done

echo ""
echo "Cluster started successfully!"
echo "PIDs saved to: $PID_FILE"
echo ""
echo "To view logs:"
echo "  tail -f /tmp/ddl-node-*.log"
echo ""
echo "To stop cluster:"
echo "  ./scripts/stop-cluster.sh"
