#!/bin/bash
#
# Stop a DDL cluster
#

set -e

PID_FILE="/tmp/ddl-cluster-pids.txt"

if [ ! -f "$PID_FILE" ]; then
	echo "No cluster PIDs file found at $PID_FILE"
	echo "Cluster may not be running"
	exit 0
fi

echo "Stopping DDL cluster..."

# Read and kill PIDs
while read -r pid; do
	if ps -p "$pid" >/dev/null 2>&1; then
		echo "Stopping node (PID: $pid)..."
		kill "$pid" 2>/dev/null || true
		wait "$pid" 2>/dev/null || true
	fi
done <"$PID_FILE"

# Clean up
rm -f "$PID_FILE"

echo "Cluster stopped successfully!"
echo ""

# Optionally clean up data directories
read -p "Clean up data directories? [y/N]: " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
	rm -rf /tmp/ddl-data-*
	rm -f /tmp/ddl-node-*.log
	echo "Data directories cleaned up"
fi
