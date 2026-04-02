#!/usr/bin/env bash
# Deploy DDL cluster across multiple remote nodes
# This script runs FROM a control machine and deploys to all cluster nodes via SSH
#
# Usage: ./deploy-cluster-remote.sh --spec "ares-comp-[13-16]" --port-base 7000
#
# Prerequisites:
#   - SSH access to all cluster nodes (passwordless recommended)
#   - ddl-node binary built on all nodes (or copied during deployment)
#   - start-cluster-remote.sh copied to all nodes

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
SSH_USER="${USER:-neeraj}"
DATA_DIR="./ddl-data"
BINARY_PATH="./target/release/ddl-node"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEPLOY_DIR="/tmp/ddl-deploy"
LOG_DIR="/tmp/ddl-logs"
WAIT_TIMEOUT=30

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
	--user)
		SSH_USER="$2"
		shift 2
		;;
	--data-dir)
		DATA_DIR="$2"
		shift 2
		;;
	--wait-timeout)
		WAIT_TIMEOUT="$2"
		shift 2
		;;
	--help | -h)
		cat <<EOF
DDL Cluster Remote Deployment Script

This script deploys DDL cluster nodes across multiple remote machines via SSH.

USAGE:
    $0 --spec <SPEC> [OPTIONS]

REQUIRED:
    --spec <SPEC>          Node specification (e.g., "ares-comp-[13-16]")

OPTIONS:
    --port-base <PORT>     Base port for coordination (default: 7000)
    --bootstrap-id <ID>    Node ID of bootstrap node (default: 1)
    --user <USER>          SSH username (default: current user)
    --data-dir <PATH>      Remote data directory (default: ./ddl-data)
    --wait-timeout <SEC>   Seconds to wait for bootstrap (default: 30)
    --help                 Show this help message

SPECIFICATION FORMATS:
    Range:     ares-comp-[13-16]     → ares-comp-13 through ares-comp-16
    List:      host1,host2,host3    → host1, host2, host3
    Simple:    single-host          → single-host

EXAMPLES:
    # Deploy cluster on ares-comp-13 through ares-comp-16
    $0 --spec "ares-comp-[13-16]"

    # Deploy with custom user and port
    $0 --spec "node[1-5]" --user admin --port-base 8000

    # Deploy comma-separated hosts
    $0 --spec "prod-node-1,prod-node-2,prod-node-3" --user deploy

PREREQUISITES:
    - SSH access configured (passwordless recommended)
    - ddl-node binary available on all nodes
    - Scripts copied to all nodes

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
	echo "Usage: $0 --spec 'ares-comp-[13-16]' [--port-base 7000] [--user <username>]"
	exit 1
fi

echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}DDL Cluster Remote Deployment${NC}"
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

echo -e "${GREEN}Deployment Configuration:${NC}"
echo "  Node specification: $NODE_SPEC"
echo "  Total nodes: $NODE_COUNT"
echo "  Nodes: ${NODES[*]}"
echo "  Bootstrap ID: $BOOTSTRAP_ID"
echo "  Port base: $PORT_BASE"
echo "  SSH user: $SSH_USER"
echo "  Data directory: $DATA_DIR"
echo "  Wait timeout: ${WAIT_TIMEOUT}s"
echo ""

# Store PIDs for parallel monitoring
declare -a SSH_PIDS
declare -a NODE_LOGS

# Cleanup function
cleanup() {
	if [ ${#SSH_PIDS[@]} -gt 0 ]; then
		echo ""
		echo -e "${YELLOW}Cleaning up SSH connections...${NC}"
		for pid in "${SSH_PIDS[@]}"; do
			if kill -0 "$pid" 2>/dev/null; then
				kill "$pid" 2>/dev/null || true
			fi
		done
	fi
}

trap cleanup EXIT

# Test SSH connectivity
echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}Testing SSH Connectivity${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""

SSH_OK=true
for node in "${NODES[@]}"; do
	echo -e -n "  Testing connection to ${node}... "
	if ssh -o ConnectTimeout=5 -o BatchMode=yes "${SSH_USER}@${node}" "echo 'ok'" >/dev/null 2>&1; then
		echo -e "${GREEN}✓${NC}"
	else
		echo -e "${RED}✗${NC}"
		echo -e "    ${YELLOW}Warning: Cannot connect to ${node}${NC}"
		SSH_OK=false
	fi
done

if ! $SSH_OK; then
	echo ""
	echo -e "${RED}Error: SSH connectivity issues detected${NC}"
	echo "Please ensure:"
	echo "  1. SSH is configured for passwordless access"
	echo "  2. All hosts are reachable"
	echo "  3. Username is correct (${SSH_USER})"
	echo ""
	echo "You can test connectivity manually:"
	for node in "${NODES[@]}"; do
		echo "  ssh ${SSH_USER}@${node} 'echo ok'"
	done
	exit 1
fi

echo ""

# Copy deployment script to all nodes
echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}Copying Deployment Scripts${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""

for node in "${NODES[@]}"; do
	echo -e -n "  Copying to ${node}... "
	ssh "${SSH_USER}@${node}" "mkdir -p $DEPLOY_DIR $LOG_DIR" 2>/dev/null
	scp "$SCRIPT_DIR/start-cluster-remote.sh" "${SSH_USER}@${node}:${DEPLOY_DIR}/start-cluster-remote.sh" 2>/dev/null
	ssh "${SSH_USER}@${node}" "chmod +x ${DEPLOY_DIR}/start-cluster-remote.sh" 2>/dev/null
	echo -e "${GREEN}✓${NC}"
done

echo ""

# Start bootstrap node first
BOOTSTRAP_NODE="${NODES[$((BOOTSTRAP_ID - 1))]}"
BOOTSTRAP_PORT=$((PORT_BASE + BOOTSTRAP_ID - 1))

echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}Starting Bootstrap Node${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""
echo -e "${GREEN}Starting bootstrap node on ${BOOTSTRAP_NODE} (Node ${BOOTSTRAP_ID})${NC}"
echo "  Port: ${BOOTSTRAP_PORT}"
echo "  Log: ${LOG_DIR}/ddl-node-${BOOTSTRAP_NODE}.log"
echo ""

# Start bootstrap node in background
ssh "${SSH_USER}@${BOOTSTRAP_NODE}" \
	"cd \$(pwd) && ${DEPLOY_DIR}/start-cluster-remote.sh --spec '$NODE_SPEC' --port-base $PORT_BASE --bootstrap-id $BOOTSTRAP_ID --data-dir '$DATA_DIR'" \
	>"${LOG_DIR}/ddl-node-${BOOTSTRAP_NODE}.log" 2>&1 &
BOOTSTRAP_PID=$!
SSH_PIDS+=($BOOTSTRAP_PID)
NODE_LOGS+=("${LOG_DIR}/ddl-node-${BOOTSTRAP_NODE}.log")

echo -e "${YELLOW}Waiting ${WAIT_TIMEOUT} seconds for bootstrap to initialize...${NC}"
sleep $WAIT_TIMEOUT

# Check if bootstrap is still running
if ! kill -0 $BOOTSTRAP_PID 2>/dev/null; then
	echo -e "${RED}Error: Bootstrap node failed to start!${NC}"
	echo "Check log: ${LOG_DIR}/ddl-node-${BOOTSTRAP_NODE}.log"
	exit 1
fi

echo -e "${GREEN}Bootstrap node appears to be running${NC}"
echo ""

# Start joining nodes in parallel
echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}Starting Joining Nodes${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""

for node in "${NODES[@]}"; do
	# Skip bootstrap node
	if [ "$node" == "$BOOTSTRAP_NODE" ]; then
		continue
	fi

	# Calculate node ID
	NODE_ID=""
	for i in "${!NODES[@]}"; do
		if [ "${NODES[$i]}" == "$node" ]; then
			NODE_ID=$((i + 1))
			break
		fi
	done

	NODE_PORT=$((PORT_BASE + NODE_ID - 1))

	echo -e "${GREEN}Starting node ${NODE_ID} on ${node}${NC}"
	echo "  Port: ${NODE_PORT}"
	echo "  Connecting to bootstrap at ${BOOTSTRAP_NODE}:${BOOTSTRAP_PORT}"

	# Start node in background
	ssh "${SSH_USER}@${node}" \
		"cd \$(pwd) && ${DEPLOY_DIR}/start-cluster-remote.sh --spec '$NODE_SPEC' --port-base $PORT_BASE --bootstrap-id $BOOTSTRAP_ID --data-dir '$DATA_DIR'" \
		>"${LOG_DIR}/ddl-node-${node}.log" 2>&1 &
	NODE_PID=$!
	SSH_PIDS+=($NODE_PID)
	NODE_LOGS+=("${LOG_DIR}/ddl-node-${node}.log")

	echo "  PID: $NODE_PID"
	echo "  Log: ${LOG_DIR}/ddl-node-${node}.log"

	# Small delay between node starts
	sleep 2
done

echo ""

# Give nodes time to connect
echo -e "${YELLOW}Waiting ${WAIT_TIMEOUT} seconds for nodes to join cluster...${NC}"
sleep $WAIT_TIMEOUT

echo ""

# Final status
echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}Cluster Deployment Complete${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""
echo -e "${GREEN}cluster is running on ${NODE_COUNT} nodes:${NC}"
for node in "${NODES[@]}"; do
	echo "  - ${node}"
done
echo ""

echo -e "${YELLOW}To check cluster status:${NC}"
echo ""
echo "  # Verify nodes are listening on coordination ports:"
for node in "${NODES[@]}"; do
	PORT_START=$((PORT_BASE))
	PORT_END=$((PORT_BASE + NODE_COUNT - 1))
	echo "  ssh ${SSH_USER}@${node} 'ss -tlnp | grep ${PORT_BASE}'"
done
echo ""

echo -e "${YELLOW}To check logs:${NC}"
echo ""
for node in "${NODES[@]}"; do
	echo "  ssh ${SSH_USER}@${node} 'tail -f ${LOG_DIR}/ddl-node-${node}.log'"
done
echo ""

echo -e "${YELLOW}To stop all nodes:${NC}"
echo ""
echo "  ./scripts/stop-cluster-remote.sh --spec '$NODE_SPEC' --user ${SSH_USER}"
echo ""

echo -e "${YELLOW}To monitor cluster health:${NC}"
echo ""
echo "  # Check cluster membership"
echo "  # (If DDL provides a status endpoint, query it here)"
echo ""

echo -e "${BLUE}================================================================================${NC}"
echo -e "${BLUE}Deployment Summary${NC}"
echo -e "${BLUE}================================================================================${NC}"
echo ""
echo "Bootstrap Node: ${BOOTSTRAP_NODE} (ID: ${BOOTSTRAP_ID}, Port: ${BOOTSTRAP_PORT})"
echo "Joining Nodes:"
for node in "${NODES[@]}"; do
	if [ "$node" != "$BOOTSTRAP_NODE" ]; then
		NODE_ID=""
		for i in "${!NODES[@]}"; do
			if [ "${NODES[$i]}" == "$node" ]; then
				NODE_ID=$((i + 1))
				break
			fi
		done
		NODE_PORT=$((PORT_BASE + NODE_ID - 1))
		echo "  ${node} (ID: ${NODE_ID}, Port: ${NODE_PORT})"
	fi
done
echo ""
