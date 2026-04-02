#!/usr/bin/env bash
# Check DDL cluster status on remote nodes
# This script checks if DDL nodes are running and healthy
#
# Usage: ./status-cluster-remote.sh --spec "ares-comp-[13-16]" --port-base 7000 --user neeraj

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
SSH_USER="${USER:-neeraj}"
CHECK_PROCESSES=true
CHECK_PORTS=true
CHECK_LOGS=false
LOG_LINES=5

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
	--user)
		SSH_USER="$2"
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
DDL Cluster Status Checker

This script checks the status of DDL cluster nodes.

USAGE:
    $0 --spec <SPEC> [OPTIONS]

REQUIRED:
    --spec <SPEC>          Node specification (e.g., "ares-comp-[13-16]")

OPTIONS:
    --port-base <PORT>     Base port for coordination (default: 7000)
    --user <USER>          SSH username (default: current user)
    --logs                 Show recent log entries
    --log-lines <N>        Number of log lines to show (default: 5)
    --help                 Show this help message

EXAMPLES:
    # Check cluster status
    $0 --spec "ares-comp-[13-16]"

    # Check with logs
    $0 --spec "node[1-5]" --logs

    # Check with more log lines
    $0 --spec "node[1-5]" --logs --log-lines 20

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
	echo "Usage: $0 --spec 'ares-comp-[13-16]' [--user <username>]"
	exit 1
fi

echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}DDL Cluster Status Check${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""

# Parse node specification
parse_node_spec() {
	local spec="$1"

	if [[ "$spec" =~ \[([0-9]+)-([0-9]+)\] ]]; then
		local prefix="${spec%\[*}"
		local start="${BASH_REMATCH[1]}"
		local end="${BASH_REMATCH[2]}"

		for i in $(seq "$start" "$end"); do
			echo "${prefix}${i}"
		done
	elif [[ "$spec" =~ , ]]; then
		echo "$spec" | tr ',' '\n'
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

echo -e "${GREEN}Cluster: ${NODE_COUNT} nodes${NC}"
echo "Specification: $NODE_SPEC"
echo "Port base: $PORT_BASE"
echo ""

# Check each node
echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}Node Status${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""

for node in "${NODES[@]}"; do
	# Get node ID
	NODE_ID=""
	for i in "${!NODES[@]}"; do
		if [ "${NODES[$i]}" == "$node" ]; then
			NODE_ID=$((i + 1))
			break
		fi
	done
	NODE_PORT=$((PORT_BASE + NODE_ID - 1))

	echo -e "${GREEN}Node ${NODE_ID}: ${node}${NC} (Port: ${NODE_PORT})"

	# Check process
	PROCESS_COUNT=$(ssh "${SSH_USER}@${node}" "pgrep -f ddl-node | wc -l || echo 0" 2>/dev/null || echo "0")

	if [ "$PROCESS_COUNT" -gt 0 ]; then
		echo -e "  Process: ${GREEN}✓ Running${NC} (${PROCESS_COUNT} process(es))"
	else
		echo -e "  Process: ${RED}✗ Not running${NC}"
	fi

	# Check port
	PORT_CHECK=$(ssh "${SSH_USER}@${node}" "ss -tlnp 2>/dev/null | grep ':${NODE_PORT}' || true" 2>/dev/null || echo "")

	if [ -n "$PORT_CHECK" ]; then
		echo -e "  Port ${NODE_PORT}: ${GREEN}✓ Listening${NC}"
	else
		echo -e "  Port ${NODE_PORT}: ${RED}✗ Not listening${NC}"
	fi

	# Check recent logs if requested
	if $CHECK_LOGS; then
		LOG_FILE="/tmp/ddl-logs/ddl-node-${node}.log"
		if ssh "${SSH_USER}@${node}" "[ -f ${LOG_FILE} ]" 2>/dev/null; then
			echo -e "  Recent logs:"
			ssh "${SSH_USER}@${node}" "tail -n ${LOG_LINES} ${LOG_FILE}" 2>/dev/null | sed 's/^/    /'
		else
			echo -e "  Logs: ${YELLOW}No log file found${NC}"
		fi
	fi

	echo ""
done

# Summary
echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}Summary${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""

TOTAL_RUNNING=0
for node in "${NODES[@]}"; do
	PROCESS_COUNT=$(ssh "${SSH_USER}@${node}" "pgrep -f ddl-node | wc -l || echo 0" 2>/dev/null || echo "0")
	if [ "$PROCESS_COUNT" -gt 0 ]; then
		TOTAL_RUNNING=$((TOTAL_RUNNING + 1))
	fi
done

if [ $TOTAL_RUNNING -eq $NODE_COUNT ]; then
	echo -e "${GREEN}All ${NODE_COUNT} nodes are running${NC}"
else
	echo -e "${YELLOW}${TOTAL_RUNNING}/${NODE_COUNT} nodes are running${NC}"
fi

echo ""
echo -e "${YELLOW}Commands:${NC}"
echo "  To stop cluster:    ./scripts/stop-cluster-remote.sh --spec '$NODE_SPEC'"
echo "  To restart cluster: ./scripts/deploy-cluster-remote.sh --spec '$NODE_SPEC'"
echo "  To view logs:      ssh ${SSH_USER}@${node} 'tail -f /tmp/ddl-logs/ddl-node-${node}.log'"
echo ""
