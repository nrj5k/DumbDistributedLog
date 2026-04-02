#!/usr/bin/env bash
# Utilities for DDL cluster deployment
# This file contains shared functions used by deployment scripts

# Parse node specification into array of hostnames
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

# Check if a command exists
command_exists() {
	command -v "$1" >/dev/null 2>&1
}

# Wait for a port to be available
wait_for_port() {
	local host="$1"
	local port="$2"
	local timeout="${3:-30}"

	local start_time=$(date +%s)
	local end_time=$((start_time + timeout))

	while [ $(date +%s) -lt $end_time ]; do
		if nc -z "$host" "$port" 2>/dev/null; then
			return 0
		fi
		sleep 1
	done

	return 1
}

# Get node ID from hostname for a given spec
get_node_id() {
	local spec="$1"
	local hostname="$2"

	mapfile -t NODES < <(parse_node_spec "$spec")

	for i in "${!NODES[@]}"; do
		if [ "${NODES[$i]}" == "$hostname" ]; then
			echo $((i + 1)) # 1-indexed
			return 0
		fi
	done

	return 1
}

# Generate peer list for a node (excludes self)
generate_peer_list() {
	local spec="$1"
	local my_id="$2"
	local port_base="$3"

	mapfile -t NODES < <(parse_node_spec "$spec")

	local peers=""
	for i in "${!NODES[@]}"; do
		local node_id=$((i + 1))
		if [ "$node_id" -ne "$my_id" ]; then
			local node_port=$((port_base + node_id - 1))
			if [ -n "$peers" ]; then
				peers="${peers},"
			fi
			peers="${peers}${node_id}@${NODES[$i]}:${node_port}"
		fi
	done

	echo "$peers"
}
