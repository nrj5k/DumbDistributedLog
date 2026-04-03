#!/usr/bin/env bash
# Massive stress test for DDL cluster using parallel-ssh
# Usage: ./scripts/massive-stress-test.sh <duration_sec> <threads_per_node>
DURATION=${1:-60}
THREADS=${2:-64}
NODES=${NODES:-"ares-comp-13 ares-comp-14 ares-comp-15 ares-comp-16"}
USER="nrajesh"
LOG_DIR="$HOME/ddl-stress-$(date +%Y%m%d_%H%M%S)"
echo "================================================================================"
echo "DDL MASSIVE STRESS TEST"
echo "================================================================================"
echo "Duration: ${DURATION}s"
echo "Threads per node: ${THREADS}"
echo "Nodes: ${NODES}"
echo "Log directory: ${LOG_DIR}"
echo ""
# Create log directory
mkdir -p "${LOG_DIR}"

# ============================================================================
# Phase 1: Pre-flight checks
# ============================================================================
echo "Phase 1: Pre-flight checks..."
echo ""

# Check all nodes are accessible
echo "Checking node accessibility..."
pssh -H "${NODES}" -l "${USER}" -i "hostname" || {
	echo "ERROR: Not all nodes are accessible"
	exit 1
}

# Check ddl-node is running on all nodes
echo "Checking ddl-node processes..."
pssh -H "${NODES}" -l "${USER}" -i "pgrep -c ddl-node" || {
	echo "ERROR: Some nodes don't have ddl-node running"
	echo "Start the cluster first: ./scripts/jumpproxy-start.sh"
	exit 1
}

# Check coordination TCP listeners
echo "Checking coordination TCP ports..."
COORD_PORTS_OK=true
for port in 7000 7001 7002 7003; do
	echo -n "  Port $port: "
	if pssh -H "${NODES}" -l "${USER}" -i "ss -tlnp | grep -q :${port}" >/dev/null 2>&1; then
		echo "✓"
	else
		echo "✗"
		COORD_PORTS_OK=false
	fi
done

if [ "$COORD_PORTS_OK" = false ]; then
	echo "ERROR: Some coordination ports are not listening"
	exit 1
fi

# Check client API TCP listeners and test PING
echo "Checking client API TCP ports..."
API_PORTS_OK=true
API_PORTS=(8080 8081 8082 8083)
NODE_ARRAY=(${NODES})
for i in "${!NODE_ARRAY[@]}"; do
	node="${NODE_ARRAY[$i]}"
	port="${API_PORTS[$i]}"
	echo -n "  ${node}:${port} - "

	# Check port is listening
	if ssh "${USER}@${node}" "ss -tlnp | grep -q :${port}" >/dev/null 2>&1; then
		echo -n "listening ✓ - "
		# Test PING command
		response=$(echo "PING" | nc -w 1 "${node}" "${port}" 2>/dev/null)
		if [[ "$response" == "PONG" ]]; then
			echo "PING ✓"
		else
			echo "PING ✗ (response: ${response:-TIMEOUT})"
			API_PORTS_OK=false
		fi
	else
		echo "not listening ✗"
		API_PORTS_OK=false
	fi
done

if [ "$API_PORTS_OK" = false ]; then
	echo "ERROR: Some client API ports are not responding"
	exit 1
fi

echo "✓ Pre-flight checks passed"
echo ""

# ============================================================================
# Phase 2: Collect baseline metrics
# ============================================================================
echo "Phase 2: Collecting baseline metrics..."
echo ""
METRICS_BASELINE="${LOG_DIR}/metrics_baseline.txt"
echo "Timestamp: $(date -Iseconds)" >"${METRICS_BASELINE}"
echo "" >>"${METRICS_BASELINE}"
for node in ${NODES}; do
	echo "=== ${node} ===" >>"${METRICS_BASELINE}"
	ssh "${USER}@${node}" "ps aux | grep ddl-node | head -1" >>"${METRICS_BASELINE}"
	ssh "${USER}@${node}" "ss -s" >>"${METRICS_BASELINE}"
	ssh "${USER}@${node}" "free -m" >>"${METRICS_BASELINE}"
	echo "" >>"${METRICS_BASELINE}"
done
echo "✓ Baseline metrics collected"
echo ""

# ============================================================================
# Phase 3: Run stress test in parallel
# ============================================================================
echo "Phase 3: Running stress test (${DURATION}s, ${THREADS} threads per node)..."
echo ""

STRESS_SCRIPT="${LOG_DIR}/stress_worker.sh"
cat >"${STRESS_SCRIPT}" <<'WORKER_EOF'
#!/usr/bin/env bash
# Stress worker script - runs actual client API operations

NODE_ID=$1
DURATION=$2
THREADS=$3
API_HOST=$4
API_PORT=$5

THREAD_DIR="/tmp/stress_threads_node_${NODE_ID}"
mkdir -p "${THREAD_DIR}"

# Helper function to send command to API
send_command() {
    local cmd="$1"
    local response
    response=$(echo "${cmd}" | nc -w 1 "${API_HOST}" "${API_PORT}" 2>/dev/null)
    echo "${response:-TIMEOUT}"
}

# Worker thread function
run_stress() {
    local thread_id=$1
    local thread_file="${THREAD_DIR}/thread-${thread_id}.log"
    local thread_ops=0
    local thread_errors=0
    local response topic resource ttl op
    
    local END_TIME=$(($(date +%s) + DURATION))
    
    while [ $(date +%s) -lt $END_TIME ]; do
        # Choose random operation
        op=$((RANDOM % 10))
        
        case $op in
            0|1|2)  # CLAIM (30%)
                topic="stress-$RANDOM-topic-$thread_id"
                response=$(send_command "CLAIM ${topic}")
                if [[ "$response" == OK* ]]; then
                    thread_ops=$((thread_ops + 1))
                else
                    thread_errors=$((thread_errors + 1))
                fi
                ;;
            3|4)  # RELEASE (20%)
                topic="stress-$RANDOM-topic-$((thread_id % 100))"
                response=$(send_command "RELEASE ${topic}")
                if [[ "$response" == OK* ]]; then
                    thread_ops=$((thread_ops + 1))
                else
                    thread_errors=$((thread_errors + 1))
                fi
                ;;
            5|6|7)  # GET_OWNER (30%)
                topic="stress-$RANDOM-topic-$((thread_id % 100))"
                response=$(send_command "GET_OWNER ${topic}")
                if [[ "$response" == OK* ]]; then
                    thread_ops=$((thread_ops + 1))
                else
                    thread_errors=$((thread_errors + 1))
                fi
                ;;
            8)  # ACQUIRE_LEASE (10%)
                resource="resource-$RANDOM-$thread_id"
                ttl=$((RANDOM % 60 + 10))
                response=$(send_command "ACQUIRE_LEASE ${resource} ${ttl}")
                if [[ "$response" == OK* ]]; then
                    thread_ops=$((thread_ops + 1))
                else
                    thread_errors=$((thread_errors + 1))
                fi
                ;;
            9)  # RELEASE_LEASE (10%)
                resource="resource-$RANDOM-$thread_id"
                response=$(send_command "RELEASE_LEASE ${resource}")
                if [[ "$response" == OK* ]]; then
                    thread_ops=$((thread_ops + 1))
                else
                    thread_errors=$((thread_errors + 1))
                fi
                ;;
        esac
        
        # Small delay to avoid overwhelming (0-4ms)
        sleep 0.00$((RANDOM % 5))
    done
    
    echo "${thread_ops} ${thread_errors}" > "${thread_file}"
}

# Main worker logic
echo "Node ${NODE_ID}: Starting stress test with ${THREADS} threads for ${DURATION}s..."
echo "  API endpoint: ${API_HOST}:${API_PORT}"

START_TIME=$(date +%s)

# Launch all worker threads
for i in $(seq 1 ${THREADS}); do
    run_stress "$i" &
done

# Wait for all threads to complete
wait

# Aggregate results from all threads
TOTAL_OPS=0
TOTAL_ERRORS=0
for i in $(seq 1 ${THREADS}); do
    thread_file="${THREAD_DIR}/thread-${i}.log"
    if [ -f "${thread_file}" ]; then
        read ops errors < "${thread_file}"
        TOTAL_OPS=$((TOTAL_OPS + ops))
        TOTAL_ERRORS=$((TOTAL_ERRORS + errors))
    fi
done

ELAPSED=$(($(date +%s) - START_TIME))
THROUGHPUT=0
if [ $ELAPSED -gt 0 ]; then
    THROUGHPUT=$((TOTAL_OPS / ELAPSED))
fi

ERROR_RATE=0
if [ $TOTAL_OPS -gt 0 ]; then
    ERROR_RATE=$((TOTAL_ERRORS * 100 / (TOTAL_OPS + TOTAL_ERRORS)))
fi

echo "Node ${NODE_ID}: Completed ${TOTAL_OPS} operations in ${ELAPSED}s (${THROUGHPUT} ops/sec)"
echo "Node ${NODE_ID}: Total errors: ${TOTAL_ERRORS} (${ERROR_RATE}% error rate)"

# Output results for aggregation
echo "${NODE_ID} ${TOTAL_OPS} ${TOTAL_ERRORS} ${ELAPSED}" > "${THREAD_DIR}/results.txt"
WORKER_EOF

chmod +x "${STRESS_SCRIPT}"

# Copy stress script to all nodes
echo "Copying stress script to nodes..."
pscp -H "${NODES}" -l "${USER}" "${STRESS_SCRIPT}" "/tmp/stress_worker.sh"

# Run stress test in parallel on all nodes
echo "Starting parallel stress test..."

# Build node-to-port mapping and launch
NODE_ARRAY=(${NODES})
API_PORTS=(8080 8081 8082 8083)
SSH_CMDS=()

for i in "${!NODE_ARRAY[@]}"; do
	node="${NODE_ARRAY[$i]}"
	port="${API_PORTS[$i]}"
	node_num=$((i + 1))
	SSH_CMDS+=("ssh ${USER}@${node} \"bash /tmp/stress_worker.sh ${node_num} ${DURATION} ${THREADS} ${node} ${port}\"")
done

# Run all stress tests in parallel
for cmd in "${SSH_CMDS[@]}"; do
	$cmd &
done

# Monitor during stress test
MONITOR_PID=$!
echo "Stress test running (PID: ${MONITOR_PID})..."
echo ""

# Collect metrics during test
METRICS_DURING="${LOG_DIR}/metrics_during.txt"
for i in $(seq 1 $((DURATION / 5))); do
	echo "=== Sample $i ($(date -Iseconds)) ===" >>"${METRICS_DURING}"
	pssh -H "${NODES}" -l "${USER}" -i "ss -s | head -5" >>"${METRICS_DURING}"
	sleep 5
done

# Wait for all background stress tests
wait

echo ""
echo "✓ Stress test completed"
echo ""

# ============================================================================
# Phase 4: Collect post-metrics
# ============================================================================
echo "Phase 4: Collecting post-test metrics..."
echo ""
METRICS_POST="${LOG_DIR}/metrics_post.txt"
echo "Timestamp: $(date -Iseconds)" >"${METRICS_POST}"
echo "" >>"${METRICS_POST}"
for node in ${NODES}; do
	echo "=== ${node} ===" >>"${METRICS_POST}"
	ssh "${USER}@${node}" "ps aux | grep ddl-node | head -1" >>"${METRICS_POST}"
	ssh "${USER}@${node}" "ss -s" >>"${METRICS_POST}"
	echo "" >>"${METRICS_POST}"
done
echo "✓ Post-metrics collected"
echo ""

# ============================================================================
# Phase 5: Gather logs
# ============================================================================
echo "Phase 5: Gathering logs from all nodes..."
echo ""
mkdir -p "${LOG_DIR}/node_logs"
for node in ${NODES}; do
	echo "  ${node}..."
	ssh "${USER}@${node}" "tar czf /tmp/ddl_logs_${node}.tar.gz -C /tmp ddl_logs/*.log 2>/dev/null || true" || true
	scp "${USER}@${node}:/tmp/ddl_logs_${node}.tar.gz" "${LOG_DIR}/node_logs/" || true
done
echo "✓ Logs gathered"
echo ""

# ============================================================================
# Phase 6: Generate summary with totals
# ============================================================================
echo "Phase 6: Generating summary..."
echo ""
SUMMARY="${LOG_DIR}/summary.txt"
echo "================================================================================" >"${SUMMARY}"
echo "DDL STRESS TEST SUMMARY" >>"${SUMMARY}"
echo "================================================================================" >>"${SUMMARY}"
echo "Date: $(date)" >>"${SUMMARY}"
echo "Duration: ${DURATION}s" >>"${SUMMARY}"
echo "Threads per node: ${THREADS}" >>"${SUMMARY}"
echo "Nodes: ${NODES}" >>"${SUMMARY}"
echo "" >>"${SUMMARY}"

echo "Results per node:" >>"${SUMMARY}"
echo "-------------------" >>"${SUMMARY}"

TOTAL_ALL_OPS=0
TOTAL_ALL_ERRORS=0
TOTAL_ALL_ELAPSED=0
NODE_COUNT=0

# Process results from each node
for node in ${NODES}; do
	result_file="/tmp/stress_threads_node_*/results.txt"
	node_result=$(ssh "${USER}@${node}" "cat ${result_file} 2>/dev/null || echo '0 0 0 0'")

	if [ -n "$node_result" ]; then
		read node_id ops errors elapsed <<<"$node_result"
		if [ -n "$ops" ] && [ "$ops" -gt 0 ]; then
			echo "Node ${node}: ${ops} ops, ${errors} errors, ${elapsed}s elapsed" >>"${SUMMARY}"
			TOTAL_ALL_OPS=$((TOTAL_ALL_OPS + ops))
			TOTAL_ALL_ERRORS=$((TOTAL_ALL_ERRORS + errors))
			TOTAL_ALL_ELAPSED=$((TOTAL_ALL_ELAPSED + elapsed))
			NODE_COUNT=$((NODE_COUNT + 1))
		fi
	fi
done

echo "" >>"${SUMMARY}"
echo "================================================================================" >>"${SUMMARY}"
echo "AGGREGATED TOTALS" >>"${SUMMARY}"
echo "================================================================================" >>"${SUMMARY}"
echo "Total operations across all nodes: ${TOTAL_ALL_OPS}" >>"${SUMMARY}"
echo "Total errors across all nodes: ${TOTAL_ALL_ERRORS}" >>"${SUMMARY}"

if [ $NODE_COUNT -gt 0 ] && [ $TOTAL_ALL_ELAPSED -gt 0 ]; then
	AVG_THROUGHPUT=$((TOTAL_ALL_OPS / TOTAL_ALL_ELAPSED))
	echo "Average throughput: ${AVG_THROUGHPUT} ops/sec" >>"${SUMMARY}"
	ERROR_PCT=$((TOTAL_ALL_ERRORS * 100 / (TOTAL_ALL_OPS + TOTAL_ALL_ERRORS)))
	echo "Overall error rate: ${ERROR_PCT}%" >>"${SUMMARY}"
fi

echo "" >>"${SUMMARY}"
echo "Metrics comparison:" >>"${SUMMARY}"
echo "--------------------" >>"${SUMMARY}"
diff -u "${METRICS_BASELINE}" "${METRICS_POST}" >>"${SUMMARY}" || true
echo "" >>"${SUMMARY}"
echo "================================================================================" >>"${SUMMARY}"

cat "${SUMMARY}"
echo ""
echo "================================================================================"
echo "STRESS TEST COMPLETE"
echo "================================================================================"
echo "Results saved to: ${LOG_DIR}"
echo ""
echo "To analyze:"
echo "  cat ${LOG_DIR}/summary.txt"
echo "  ls ${LOG_DIR}/"
