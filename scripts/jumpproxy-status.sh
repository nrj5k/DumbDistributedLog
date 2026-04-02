#!/usr/bin/env bash
# Check DDL cluster status via jumpproxy
# This script checks if DDL nodes are running and healthy
# Uses 'ssh ares-comp-N' (no user@ prefix)
#
# Usage: ./scripts/jumpproxy-status.sh [--spec "ares-comp-[13-16]"] [--port-base 7000] [--logs] [--log-lines N]

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
	--data-dir)
		DATA_DIR="$2"
		shift 2
		;;
	--logs)
		CHECK_LOGS=true
		shift
		;;
	--log-lines)
		LOG_LINES="$2"
		shift 2
		;;
	--help | -h)
		cat <<EOF
DDL Cluster Status (jumpproxy)

Checks status of DDL cluster nodes on remote machines via jumpproxy SSH access.

USAGE:
    $0 [OPTIONS]

OPTIONS:
    --spec <SPEC>          Node specification (default: ${DEFAULT_NODE_SPEC})
    --port-base <PORT>     Base port for coordination (default: ${PORT_BASE})
    --data-dir <PATH>      Data directory (default: ${DATA_DIR})
    --logs                 Show recent log entries
    --log-lines <N>        Number of log lines to show (default: ${LOG_LINES})
    --help                 Show this help message

SPECIFICATION FORMATS:
    Range:     ares-comp-[13-16]     → ares-comp-13 through ares-comp-16
    List:      ares-comp-13,ares-comp-14    → specific hosts
    Simple:    ares-comp-13          → single host

EXAMPLES:
    # Check with defaults from config
    $0

    # Check cluster status
    $0 --spec "ares-comp-[13-16]"

    # Check with logs
    $0 --spec "ares-comp-[13-16]" --logs

    # Check with more detailed logs
    $0 --spec "ares-comp-[13-16]" --logs --log-lines 50

NOTES:
    - Uses 'ssh ares-comp-N' (no user@ prefix)
    - Checks both process status and port availability
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
echo -e "${BLUE}DDL Cluster Status Check (jumpproxy)${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""
echo -e "${GREEN}Cluster Configuration:${NC}"
echo "  Nodes: ${NODES[*]}"
echo "  Node count: $NODE_COUNT"
echo "  Port base: $PORT_BASE"
echo "  Data directory: $DATA_DIR"
echo "  Config file: $CONFIG_FILE"
echo ""

# Check each node
echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}Node Status${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""

TOTAL_RUNNING=0
TOTAL_LISTENING=0

for i in "${!NODES[@]}"; do
	NODE_ID=$((i + 1))
	NODE="${NODES[$i]}"
	NODE_PORT=$((PORT_BASE + NODE_ID - 1))

	echo -e "${GREEN}Node ${NODE_ID}: ${NODE}${NC}"
	echo "  Port: ${NODE_PORT}"

	# Check process
	PROCESS_COUNT=$(ssh "${NODE}" "pgrep -f ddl-node | wc -l || echo 0" 2>/dev/null || echo "0")

	if [ "$PROCESS_COUNT" -gt 0 ]; then
		echo -e "  Process: ${GREEN}✓ Running${NC} (${PROCESS_COUNT} process(es))"
		TOTAL_RUNNING=$((TOTAL_RUNNING + 1))
	else
		echo -e "  Process: ${RED}✗ Not running${NC}"
	fi

	# Check port
	PORT_CHECK=$(ssh "${NODE}" "ss -tlnp 2>/dev/null | grep ':${NODE_PORT}' || true" 2>/dev/null || echo "")

	if [ -n "$PORT_CHECK" ]; then
		echo -e "  Port ${NODE_PORT}: ${GREEN}✓ Listening${NC}"
		TOTAL_LISTENING=$((TOTAL_LISTENING + 1))
	else
		echo -e "  Port ${NODE_PORT}: ${RED}✗ Not listening${NC}"
	fi

	# Show logs if requested
	if $CHECK_LOGS; then
		LOG_FILE="${DATA_DIR}/node-${NODE_ID}/ddl-node.log"

		if ssh "${NODE}" "[ -f ${LOG_FILE} ]" 2>/dev/null; then
			echo -e "  Recent logs:"
			ssh "${NODE}" "tail -n ${LOG_LINES} ${LOG_FILE}" 2>/dev/null | sed 's/^/    /'
		else
			echo -e "  Logs: ${YELLOW}No log file at ${LOG_FILE}${NC}"
		fi
	fi

	echo ""
done

# Summary
echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}Summary${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""

if [ $TOTAL_RUNNING -eq $NODE_COUNT ]; then
	echo -e "Processes: ${GREEN}All ${NODE_COUNT} nodes running${NC}"
else
	echo -e "Processes: ${YELLOW}${TOTAL_RUNNING}/${NODE_COUNT} nodes running${NC}"
fi

if [ $TOTAL_LISTENING -eq $NODE_COUNT ]; then
	echo -e "Ports:     ${GREEN}All ${NODE_COUNT} ports listening${NC}"
else
	echo -e "Ports:     ${YELLOW}${TOTAL_LISTENING}/${NODE_COUNT} ports listening${NC}"
fi

echo ""

# Overall health
if [ $TOTAL_RUNNING -eq $NODE_COUNT ] && [ $TOTAL_LISTENING -eq $NODE_COUNT ]; then
	echo -e "${GREEN}================================================================================${NC}"
	echo -e "${GREEN}Cluster Status: HEALTHY${NC}"
	echo -e "${GREEN}================================================================================${NC}"
else
	echo -e "${RED}================================================================================${NC}"
	echo -e "${RED}Cluster Status: DEGRADED${NC}"
	echo -e "${RED}================================================================================${NC}"
fi

echo ""
echo -e "${YELLOW}Useful commands:${NC}"
echo "  Start cluster:  ./scripts/jumpproxy-start.sh --spec '${NODE_SPEC}'"
echo "  Stop cluster:   ./scripts/jumpproxy-stop.sh --spec '${NODE_SPEC}'"
echo "  Check logs:     $0 --spec '${NODE_SPEC}' --logs --log-lines 50"
echo ""

# Show log access commands
if ! $CHECK_LOGS; then
	echo -e "${YELLOW}To view logs on specific nodes:${NC}"
	for i in "${!NODES[@]}"; do
		NODE_ID=$((i + 1))
		NODE="${NODES[$i]}"
		echo "  Node ${NODE_ID}: ssh ${NODE} 'tail -f ${DATA_DIR}/node-${NODE_ID}/ddl-node.log'"
	done
	echo ""
fi

# Exit with appropriate code
if [ $TOTAL_RUNNING -eq $NODE_COUNT ] && [ $TOTAL_LISTENING -eq $NODE_COUNT ]; then
	exit 0
else
	exit 1
fi
