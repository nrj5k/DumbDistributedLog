#!/usr/bin/env bash
# Start DDL cluster via jumpproxy
# This script starts ddl-node on each machine via SSH jumpproxy
# Code exists at /mnt/common/nrajesh/DumbDistributedLog on remote nodes
#
# Usage: ./scripts/jumpproxy-start.sh [--spec "ares-comp-[13-16]"] [--port-base 7000] [--bootstrap-id 1]

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Load configuration
CONFIG_FILE="${SCRIPT_DIR}/cluster-config.local.sh"
if [ ! -f "$CONFIG_FILE" ]; then
	echo "ERROR: Config file not found: $CONFIG_FILE"
	echo "Please copy template and customize:"
	echo "  cp ${SCRIPT_DIR}/cluster-config.sh.template ${CONFIG_FILE}"
	exit 1
fi

source "$CONFIG_FILE"

# Load shared functions
source "${SCRIPT_DIR}/cluster-functions.sh"

# Validate configuration
if ! validate_config; then
	echo "ERROR: Configuration validation failed"
	exit 1
fi

# Default values from config
NODE_SPEC="$DEFAULT_NODE_SPEC"
CHECK_LOGS=false

# Parse command-line arguments (override config file)
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
	--remote-dir)
		REMOTE_DIR="$2"
		shift 2
		;;
	--logs)
		CHECK_LOGS=true
		shift
		;;
	--help | -h)
		cat <<EOF
DDL Cluster Startup (jumpproxy)

Starts DDL cluster nodes on remote machines via jumpproxy SSH access.

USAGE:
    $0 [OPTIONS]

OPTIONS:
    --spec <SPEC>          Node specification (default: ${DEFAULT_NODE_SPEC})
    --port-base <PORT>     Base port for coordination (default: ${PORT_BASE})
    --bootstrap-id <ID>    Node ID of bootstrap node (default: ${BOOTSTRAP_ID})
    --data-dir <PATH>      Data directory (default: ${DATA_DIR})
    --remote-dir <PATH>    Remote directory (default: ${REMOTE_DIR})
    --logs                 Show logs after startup
    --help                 Show this help message

SPECIFICATION FORMATS:
    Range:     ares-comp-[13-16]     → ares-comp-13 through ares-comp-16
    List:      ares-comp-13,ares-comp-14    → specific hosts
    Simple:    ares-comp-13          → single host

EXAMPLES:
    # Start with defaults from config
    $0

    # Start 4-node cluster with custom spec
    $0 --spec "ares-comp-[13-16]"

    # Start with custom port
    $0 --spec "ares-comp-[13-16]" --port-base 8000

NOTES:
    - Uses 'ssh ares-comp-N' (no user@ prefix)
    - Code must exist at: ${REMOTE_DIR}
    - Binary must be built: ${BINARY_PATH}
    - Config file: ${CONFIG_FILE}

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
	echo "Usage: $0 --spec 'ares-comp-[13-16]' [--port-base 7000]"
	exit 1
fi

# Parse into array
mapfile -t NODES < <(parse_node_spec "$NODE_SPEC")
NODE_COUNT=${#NODES[@]}

if [ $NODE_COUNT -eq 0 ]; then
	echo -e "${RED}Error: No nodes found in specification '$NODE_SPEC'${NC}"
	exit 1
fi

echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}DDL Cluster Startup (jumpproxy)${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""
echo -e "${GREEN}Cluster Configuration:${NC}"
echo "  Nodes: ${NODES[*]}"
echo "  Node count: $NODE_COUNT"
echo "  Port base: $PORT_BASE"
echo "  Bootstrap node: $BOOTSTRAP_ID"
echo "  Remote directory: $REMOTE_DIR"
echo "  Data directory: $DATA_DIR"
echo "  Config file: $CONFIG_FILE"
echo ""

# Start bootstrap node first (synchronously)
echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}Phase 1: Starting Bootstrap Node${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""

BOOTSTRAP_NODE="${NODES[$((BOOTSTRAP_ID - 1))]}"
BOOTSTRAP_PORT="$((PORT_BASE + BOOTSTRAP_ID - 1))"
BOOTSTRAP_DATA_DIR="${DATA_DIR}/node-${BOOTSTRAP_ID}"

echo -e "${GREEN}Node ${BOOTSTRAP_ID}: ${BOOTSTRAP_NODE} (BOOTSTRAP)${NC}"
echo "  Port: ${BOOTSTRAP_PORT}"
echo "  Data directory: ${BOOTSTRAP_DATA_DIR}"
echo ""

# Build bootstrap command (subshell + </dev/null to prevent SSH hang)
BOOTSTRAP_CMD="cd ${REMOTE_DIR} && mkdir -p ${BOOTSTRAP_DATA_DIR} && (nohup ${BINARY_PATH} --id ${BOOTSTRAP_ID} --port ${BOOTSTRAP_PORT} --host 0.0.0.0 --data-dir ${BOOTSTRAP_DATA_DIR} --bootstrap > ${BOOTSTRAP_DATA_DIR}/ddl-node.log 2>&1 </dev/null &)"

echo "  Starting bootstrap node..."
if ssh "${BOOTSTRAP_NODE}" "${BOOTSTRAP_CMD}" 2>/dev/null; then
	echo -e "  ${GREEN}✓ Bootstrap node started${NC}"
else
	echo -e "  ${RED}✗ Failed to start bootstrap node${NC}"
	exit 1
fi

# Wait for bootstrap node to initialize
echo ""
echo "Waiting for bootstrap node to initialize..."
sleep "$BOOTSTRAP_WAIT"

# Start remaining nodes in parallel
echo ""
echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}Phase 2: Starting Remaining Nodes${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""

declare -a PIDS
for i in "${!NODES[@]}"; do
	NODE_ID=$((i + 1))
	NODE="${NODES[$i]}"

	# Skip bootstrap node (already started)
	if [ "$NODE_ID" -eq "$BOOTSTRAP_ID" ]; then
		continue
	fi

	NODE_PORT=$((PORT_BASE + NODE_ID - 1))
	NODE_DATA_DIR="${DATA_DIR}/node-${NODE_ID}"
	PEERS=$(build_peer_list "$NODE_SPEC" "$NODE")

	echo -e "${GREEN}Node ${NODE_ID}: ${NODE}${NC}"
	echo "  Port: ${NODE_PORT}"
	echo "  Peers: ${PEERS}"
	echo "  Data directory: ${NODE_DATA_DIR}"

	# Build node command (subshell + </dev/null to prevent SSH hang)
	NODE_CMD="cd ${REMOTE_DIR} && mkdir -p ${NODE_DATA_DIR} && (nohup ${BINARY_PATH} --id ${NODE_ID} --port ${NODE_PORT} --host 0.0.0.0 --data-dir ${NODE_DATA_DIR} --peers ${PEERS} > ${NODE_DATA_DIR}/ddl-node.log 2>&1 </dev/null &)"

	# Start in background
	(
		if ssh "${NODE}" "${NODE_CMD}" 2>/dev/null; then
			echo -e "  ${GREEN}✓ Started${NC}"
		else
			echo -e "  ${RED}✗ Failed${NC}"
		fi
	) &

	PIDS+=($!)
done

# Wait for all background processes
echo ""
echo "Waiting for all nodes to start..."
for pid in "${PIDS[@]}"; do
	wait "$pid" || true
done

sleep "$CLUSTER_READY_WAIT"

# Verify nodes are running
echo ""
echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}Verification${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""

ALL_RUNNING=true
for i in "${!NODES[@]}"; do
	NODE_ID=$((i + 1))
	NODE="${NODES[$i]}"
	NODE_PORT=$((PORT_BASE + NODE_ID - 1))

	PROCESS_COUNT=$(ssh "${NODE}" "pgrep -f ddl-node | wc -l || echo 0" 2>/dev/null || echo "0")

	if [ "$PROCESS_COUNT" -gt 0 ]; then
		echo -e "Node ${NODE_ID} (${NODE}): ${GREEN}✓ Running${NC}"
	else
		echo -e "Node ${NODE_ID} (${NODE}): ${RED}✗ Not running${NC}"
		ALL_RUNNING=false
	fi

	# Show logs if requested
	if $CHECK_LOGS; then
		LOG_FILE="${DATA_DIR}/node-${NODE_ID}/ddl-node.log"
		if ssh "${NODE}" "[ -f ${LOG_FILE} ]" 2>/dev/null; then
			echo -e "  Recent logs:"
			ssh "${NODE}" "tail -n ${LOG_LINES} ${LOG_FILE}" 2>/dev/null | sed 's/^/    /'
		fi
	fi
done

echo ""

if $ALL_RUNNING; then
	echo -e "${GREEN}================================================================================${NC}"
	echo -e "${GREEN}All ${NODE_COUNT} nodes started successfully!${NC}"
	echo -e "${GREEN}================================================================================${NC}"
	echo ""
	echo -e "${YELLOW}Useful commands:${NC}"
	echo "  Check status: ./scripts/jumpproxy-status.sh --spec '${NODE_SPEC}'"
	echo "  Stop cluster: ./scripts/jumpproxy-stop.sh --spec '${NODE_SPEC}'"
	echo "  View logs:    ssh ${NODES[0]} 'tail -f ${DATA_DIR}/node-1/ddl-node.log'"
else
	echo -e "${RED}================================================================================${NC}"
	echo -e "${RED}Some nodes failed to start${NC}"
	echo -e "${RED}================================================================================${NC}"
	echo ""
	echo "Check logs for details:"
	for i in "${!NODES[@]}"; do
		NODE_ID=$((i + 1))
		NODE="${NODES[$i]}"
		echo "  ssh ${NODE} 'tail -n 50 ${DATA_DIR}/node-${NODE_ID}/ddl-node.log'"
	done
	exit 1
fi
