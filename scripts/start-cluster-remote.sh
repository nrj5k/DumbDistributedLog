#!/usr/bin/env bash
# Start DDL cluster node (remote deployment)
# This script runs on EACH node in the cluster
# It determines which node it is from the specification and starts ddl-node accordingly
#
# Usage: ./start-cluster-remote.sh --spec "ares-comp-[13-16]" --port-base 7000 --bootstrap-id 1
#
# The spec syntax supports:
#   - Range: "ares-comp-[13-16]" → ares-comp-13, ares-comp-14, ares-comp-15, ares-comp-16
#   - List: "host1,host2,host3" → host1, host2, host3
#   - Simple: "single-host" → single-host

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
NODE_SPEC=""
PORT_BASE=7000
BOOTSTRAP_ID=1
DATA_DIR="./ddl-data"
BINARY_PATH="./target/release/ddl-node"

# Parse arguments
while [[ $# -gt 0 ]]; do
	case $1 in
	--spec)
		NODE_SPEC="$2"
		shift 2
		;;
	--port-base)
		PORT_BASE="$2"
		shift 2
		;;
	--bootstrap-id)
		BOOTSTRAP_ID="$2"
		shift 2
		;;
	--data-dir)
		DATA_DIR="$2"
		shift 2
		;;
	--binary)
		BINARY_PATH="$2"
		shift 2
		;;
	--help | -h)
		cat <<EOF
DDL Cluster Node Starter

This script starts a single DDL node on the current machine.
It determines its node ID from the cluster specification and hostname.

USAGE:
    $0 --spec <SPEC> [OPTIONS]

REQUIRED:
    --spec <SPEC>          Node specification (e.g., "ares-comp-[13-16]" or "host1,host2")

OPTIONS:
    --port-base <PORT>     Base port for coordination (default: 7000)
                           Node N uses port (port-base + N - 1)
    --bootstrap-id <ID>    Node ID of bootstrap node (default: 1)
    --data-dir <PATH>      Data directory for persistence (default: ./ddl-data)
    --binary <PATH>        Path to ddl-node binary (default: ./target/release/ddl-node)
    --help                 Show this help message

SPECIFICATION FORMATS:
    Range:     ares-comp-[13-16]     → ares-comp-13 through ares-comp-16
    List:      host1,host2,host3    → host1, host2, host3
    Simple:    single-host          → single-host

EXAMPLES:
    # On ares-comp-13 (bootstrap):
    $ $0 --spec "ares-comp-[13-16]"

    # On ares-comp-14 (joining):
    $ $0 --spec "ares-comp-[13-16]"

    # Custom port and data directory:
    $ $0 --spec "node[1-5]" --port-base 8000 --data-dir /data/ddl

EOF
		exit 0
		;;
	*)
		echo -e "${RED}Error: Unknown option: $1${NC}"
		echo "Use --help for usage information"
		exit 1
		;;
	esac
done

# Validate required arguments
if [ -z "$NODE_SPEC" ]; then
	echo -e "${RED}Error: --spec is required${NC}"
	echo "Usage: $0 --spec 'ares-comp-[13-16]' [--port-base 7000] [--bootstrap-id 1]"
	exit 1
fi

# Get current hostname
HOSTNAME=$(hostname)
echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}DDL Cluster Node Startup${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""
echo "Hostname: $HOSTNAME"
echo "Specification: $NODE_SPEC"
echo ""

# Parse node specification
# Supports: "ares-comp-[13-16]", "node[1-5]", "host1,host2,host3"
parse_node_spec() {
	local spec="$1"

	# Check for range notation [N-M]
	if [[ "$spec" =~ \[([0-9]+)-([0-9]+)\] ]]; then
		local prefix="${spec%\[*}"
		local start="${BASH_REMATCH[1]}"
		local end="${BASH_REMATCH[2]}"

		for i in $(seq "$start" "$end"); do
			echo "${prefix}${i}"
		done
	# Check for comma-separated list
	elif [[ "$spec" =~ , ]]; then
		echo "$spec" | tr ',' '\n'
	# Single node
	else
		echo "$spec"
	fi
}

# Parse into array
mapfile -t NODES < <(parse_node_spec "$NODE_SPEC")
NODE_COUNT=${#NODES[@]}

if [ $NODE_COUNT -eq 0 ]; then
	echo -e "${RED}Error: No nodes found in specification '$NODE_SPEC'${NC}"
	exit 1
fi

echo -e "${GREEN}Discovered ${NODE_COUNT} nodes in cluster:${NC}"
for i in "${!NODES[@]}"; do
	NODE_ID=$((i + 1))
	echo "  Node ${NODE_ID}: ${NODES[$i]}"
done
echo ""

# Find this node's ID
MY_ID=""
for i in "${!NODES[@]}"; do
	if [ "${NODES[$i]}" == "$HOSTNAME" ]; then
		MY_ID=$((i + 1)) # 1-indexed
		break
	fi
done

if [ -z "$MY_ID" ]; then
	echo -e "${RED}Error: Hostname '$HOSTNAME' not found in node specification '$NODE_SPEC'${NC}"
	echo ""
	echo "Available nodes in specification:"
	for i in "${!NODES[@]}"; do
		NODE_ID=$((i + 1))
		echo "  $NODE_ID: ${NODES[$i]}"
	done
	echo ""
	echo "If you're running this manually, you can override hostname detection:"
	echo "  export FORCE_NODE_ID=<desired_id>"
	echo "  $0 --spec '$NODE_SPEC'"
	exit 1
fi

echo -e "${GREEN}Cluster Configuration:${NC}"
echo "  My Node ID: $MY_ID"
echo "  My Hostname: $HOSTNAME"
echo "  Total Nodes: $NODE_COUNT"
echo "  Bootstrap ID: $BOOTSTRAP_ID"
echo "  Port Base: $PORT_BASE"
echo ""

# Calculate my coordination port
MY_PORT=$((PORT_BASE + MY_ID - 1))

# Build peer list (all nodes except ourselves)
PEERS=""
for i in "${!NODES[@]}"; do
	NODE_ID=$((i + 1))
	if [ "$NODE_ID" -ne "$MY_ID" ]; then
		NODE_PORT=$((PORT_BASE + NODE_ID - 1))
		if [ -n "$PEERS" ]; then
			PEERS="${PEERS},"
		fi
		PEERS="${PEERS}${NODE_ID}@${NODES[$i]}:${NODE_PORT}"
	fi
done

# Determine if this is the bootstrap node
IS_BOOTSTRAP=false
if [ "$MY_ID" -eq "$BOOTSTRAP_ID" ]; then
	IS_BOOTSTRAP=true
fi

# Setup data directory
NODE_DATA_DIR="${DATA_DIR}/node-${MY_ID}"
mkdir -p "$NODE_DATA_DIR"

echo -e "${GREEN}Node Settings:${NC}"
echo "  Mode: $(if $IS_BOOTSTRAP; then echo "BOOTSTRAP (first node)"; else echo "JOINING"; fi)"
echo "  Coordination Port: $MY_PORT"
echo "  Data Directory: $NODE_DATA_DIR"
if [ -n "$PEERS" ]; then
	echo "  Peers: $PEERS"
fi
echo ""

# Check if binary exists
if [ ! -f "$BINARY_PATH" ]; then
	echo -e "${RED}Error: Binary not found at $BINARY_PATH${NC}"
	echo ""
	echo "Build the binary first:"
	echo "  cargo build --release --bin ddl-node"
	exit 1
fi

# Build ddl-node command
CMD="$BINARY_PATH"
CMD="$CMD --id $MY_ID"
CMD="$CMD --port $MY_PORT"
CMD="$CMD --host 0.0.0.0"
CMD="$CMD --data-dir $NODE_DATA_DIR"

if $IS_BOOTSTRAP; then
	CMD="$CMD --bootstrap"
else
	CMD="$CMD --peers $PEERS"
fi

echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}Starting DDL Node $MY_ID${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""
echo "Command: $CMD"
echo ""

# Start the node
exec $CMD
