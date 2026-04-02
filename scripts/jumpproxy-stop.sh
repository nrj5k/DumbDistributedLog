#!/usr/bin/env bash
# Stop DDL cluster via jumpproxy
# This script kills ddl-node processes on all remote nodes
# Uses 'ssh ares-comp-N' (no user@ prefix)
#
# Usage: ./scripts/jumpproxy-stop.sh [--spec "ares-comp-[13-16]"]

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

# Parse command-line arguments (override config file)
while [[ $# -gt 0 ]]; do
	case $1 in
	--spec)
		NODE_SPEC="$2"
		shift 2
		;;
	--data-dir)
		DATA_DIR="$2"
		shift 2
		;;
	--help | -h)
		cat <<EOF
DDL Cluster Stop (jumpproxy)

Stops DDL cluster nodes on remote machines via jumpproxy SSH access.

USAGE:
    $0 [OPTIONS]

OPTIONS:
    --spec <SPEC>          Node specification (default: ${DEFAULT_NODE_SPEC})
    --data-dir <PATH>      Data directory for cleanup info (default: ${DATA_DIR})
    --help                 Show this help message

SPECIFICATION FORMATS:
    Range:     ares-comp-[13-16]     → ares-comp-13 through ares-comp-16
    List:      ares-comp-13,ares-comp-14    → specific hosts
    Simple:    ares-comp-13          → single host

EXAMPLES:
    # Stop cluster with defaults from config
    $0

    # Stop cluster on all nodes
    $0 --spec "ares-comp-[13-16]"

NOTES:
    - Uses 'ssh ares-comp-N' (no user@ prefix)
    - Kills ddl-node processes via 'pkill -f ddl-node'
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
	echo "Usage: $0 --spec 'ares-comp-[13-16]'"
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
echo -e "${BLUE}DDL Cluster Stop (jumpproxy)${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""
echo -e "${GREEN}Stopping ${NODE_COUNT} nodes:${NC} ${NODES[*]}"
echo ""

# Stop each node
declare -a PIDS
for node in "${NODES[@]}"; do
	(
		echo -e -n "  Stopping ${node}... "

		# Try to kill ddl-node process
		if ssh "${node}" "pkill -f ddl-node || true" 2>/dev/null; then
			echo -e "${GREEN}✓${NC}"
		else
			echo -e "${YELLOW}✗ (no process found or connection failed)${NC}"
		fi
	) &

	PIDS+=($!)
done

# Wait for all stops to complete
for pid in "${PIDS[@]}"; do
	wait $pid || true
done

echo ""

# Verify nodes are stopped
echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}Verification${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""

ALL_STOPPED=true
for node in "${NODES[@]}"; do
	PROCESS_COUNT=$(ssh "${node}" "pgrep -f ddl-node | wc -l || echo 0" 2>/dev/null || echo "0")

	if [ "$PROCESS_COUNT" -eq 0 ]; then
		echo -e "${GREEN}✓${NC} ${node}: No ddl-node processes running"
	else
		echo -e "${RED}✗${NC} ${node}: ${PROCESS_COUNT} process(es) still running"
		ALL_STOPPED=false
	fi
done

echo ""

if $ALL_STOPPED; then
	echo -e "${GREEN}================================================================================${NC}"
	echo -e "${GREEN}All nodes stopped successfully${NC}"
	echo -e "${GREEN}================================================================================${NC}"
else
	echo -e "${YELLOW}================================================================================${NC}"
	echo -e "${YELLOW}Some nodes may still be running - verify manually${NC}"
	echo -e "${YELLOW}================================================================================${NC}"
	echo ""
	echo "To force kill processes:"
	for node in "${NODES[@]}"; do
		echo "  ssh ${node} 'pkill -9 -f ddl-node'"
	done
fi

echo ""
echo -e "${YELLOW}Data cleanup (optional):${NC}"
echo "To remove data directories:"
for node in "${NODES[@]}"; do
	echo "  ssh ${node} 'rm -rf ${DATA_DIR}'"
done
echo ""

echo -e "${YELLOW}Next steps:${NC}"
echo "  To restart: ./scripts/jumpproxy-start.sh --spec '${NODE_SPEC}'"
echo "  To check status: ./scripts/jumpproxy-status.sh --spec '${NODE_SPEC}'"
echo ""
