#!/usr/bin/env bash
# Stop DDL cluster on all remote nodes
# This script stops ddl-node processes on all nodes specified
#
# Usage: ./stop-cluster-remote.sh --spec "ares-comp-[13-16]" --user neeraj

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
NODE_SPEC=""
SSH_USER="${USER:-neeraj}"
DATA_DIR="./ddl-data"

# Parse arguments
while [[ $# -gt 0 ]]; do
	case $1 in
	--spec)
		NODE_SPEC="$2"
		shift 2
		;;
	--user)
		SSH_USER="$2"
		shift 2
		;;
	--data-dir)
		DATA_DIR="$2"
		shift 2
		;;
	--help | -h)
		cat <<EOF
DDL Cluster Stop Script

This script stops DDL clusters nodes on remote machines.

USAGE:
    $0 --spec <SPEC> [OPTIONS]

REQUIRED:
    --spec <SPEC>          Node specification (e.g., "ares-comp-[13-16]")

OPTIONS:
    --user <USER>          SSH username (default: current user)
    --data-dir <PATH>      Data directory to clean up (optional)
    --help                 Show this help message

SPECIFICATION FORMATS:
    Range:     ares-comp-[13-16]     → ares-comp-13 through ares-comp-16
    List:      host1,host2,host3    → host1, host2, host3
    Simple:    single-host          → single-host

EXAMPLES:
    # Stop cluster on ares-comp-13 through ares-comp-16
    $0 --spec "ares-comp-[13-16]"

    # Stop with custom user
    $0 --spec "node[1-5]" --user admin

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
echo -e "${BLUE}Stopping DDL Cluster${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""

# Parse node specification
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

echo -e "${GREEN}Stopping ${NODE_COUNT} nodes:${NC} ${NODES[*]}"
echo ""

# Stop each node
for node in "${NODES[@]}"; do
	echo -e -n "  Stopping node on ${node}... "

	# Try to kill ddl-node process
	if ssh "${SSH_USER}@${node}" "pkill -f ddl-node || true" 2>/dev/null; then
		echo -e "${GREEN}✓${NC}"
	else
		echo -e "${YELLOW}✗ (no process found or connection failed)${NC}"
	fi
done

echo ""

echo -e "${YELLOW}To verify all nodes are stopped:${NC}"
echo ""
for node in "${NODES[@]}"; do
	echo "  ssh ${SSH_USER}@${node} 'ps aux | grep ddl-node | grep -v grep'"
done
echo ""

echo -e "${YELLOW}To clean up data directories (if needed):${NC}"
echo ""
for node in "${NODES[@]}"; do
	echo "  ssh ${SSH_USER}@${node} 'rm -rf ${DATA_DIR}/node-*'"
done
echo ""

echo -e "${GREEN}All nodes stopped${NC}"
