#!/usr/bin/env bash
# Shared functions for cluster management scripts

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Load configuration if not already loaded
if [ -z "$REMOTE_DIR" ]; then
	CONFIG_FILE="${SCRIPT_DIR}/cluster-config.local.sh"
	if [ -f "$CONFIG_FILE" ]; then
		source "$CONFIG_FILE"
	else
		echo "ERROR: Config not found: $CONFIG_FILE" >&2
		return 1
	fi
fi

# Parse node specification
# Supports: "ares-comp-[13-16]", "node[1-5]", "host1,host2,host3"
parse_node_spec() {
	local spec="$1"

	# Check for range notation [N-M]
	if [[ "$spec" =~ (.*)\[([0-9]+)-([0-9]+)\] ]]; then
		local prefix="${BASH_REMATCH[1]}"
		local start="${BASH_REMATCH[2]}"
		local end="${BASH_REMATCH[3]}"

		for i in $(seq "$start" "$end"); do
			echo "${prefix}${i}"
		done
	# Check for comma-separated
	elif [[ "$spec" =~ , ]]; then
		echo "$spec" | tr ',' '\n'
	else
		echo "$spec"
	fi
}

# Build peer list for a specific node
# Arguments: node_spec, my_hostname
# Output: "id@host:port,id2@host2:port2,..."
build_peer_list() {
	local spec="$1"
	local my_host="$2"

	mapfile -t nodes < <(parse_node_spec "$spec")
	local result=""

	for i in "${!nodes[@]}"; do
		local node="${nodes[$i]}"
		local node_id=$((i + 1))
		local node_port=$((PORT_BASE + node_id - 1))

		if [ "$node" != "$my_host" ]; then
			if [ -n "$result" ]; then
				result="${result},"
			fi
			result="${result}${node_id}@${node}:${node_port}"
		fi
	done

	echo "$result"
}

# Execute command on remote node via SSH
# Arguments: hostname, command
remote_exec() {
	local host="$1"
	local cmd="$2"

	ssh "$host" "cd $REMOTE_DIR && $cmd"
}

# Check if ddl-node is running on a host
# Arguments: hostname
# Returns: 0 if running, 1 if not
check_node_running() {
	local host="$1"

	if ssh "$host" "pgrep -f 'ddl-node' > /dev/null 2>&1"; then
		return 0
	else
		return 1
	fi
}

# Get ddl-node PID from a host
# Arguments: hostname
get_node_pid() {
	local host="$1"
	ssh "$host" "pgrep -f 'ddl-node'" 2>/dev/null || echo ""
}

# Print colored status
print_status() {
	local status="$1"
	local message="$2"

	case "$status" in
	ok | OK | success | SUCCESS)
		echo -e "${GREEN}✓${NC} $message"
		;;
	error | ERROR | fail | FAIL)
		echo -e "${RED}✗${NC} $message"
		;;
	warn | WARN | warning | WARNING)
		echo -e "${YELLOW}⚠${NC} $message"
		;;
	info | INFO)
		echo -e "${BLUE}ℹ${NC} $message"
		;;
	*)
		echo "$message"
		;;
	esac
}
