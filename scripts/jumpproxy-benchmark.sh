#!/usr/bin/env bash
# DDL Benchmark Script
# Measures latency and throughput against deployed cluster

set -e

# Configuration
DURATION=${1:-30}
TARGET_NODE=${2:-"ares-comp-13:8080"}
SECONDARY_NODE=${3:-"ares-comp-14:8080"}
RESULTS_DIR="$HOME/ddl-benchmark-$(date +%Y%m%d_%H%M%S)"

# Targets
TARGET_LATENCY_CLAIM_P99=50         # ms
TARGET_LATENCY_RELEASE_P99=10       # ms
TARGET_LATENCY_QUERY_P99=5          # ms
TARGET_LATENCY_LEASE_ACQUIRE_P99=50 # ms
TARGET_LATENCY_LEASE_RELEASE_P99=10 # ms
TARGET_THROUGHPUT_CLAIMS=1000       # ops/sec
TARGET_THROUGHPUT_QUERIES=5000      # ops/sec
TARGET_THROUGHPUT_MIXED=2000        # ops/sec
TARGET_CONSENSUS_LATENCY=100        # ms

# Results storage
declare -a LATENCIES_CLAIM
declare -a LATENCIES_RELEASE
declare -a LATENCIES_QUERY
declare -a LATENCIES_LEASE_ACQUIRE
declare -a LATENCIES_LEASE_RELEASE

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Utility functions
log_info() {
	echo -e "${YELLOW}[INFO]${NC} $1"
}

log_success() {
	echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_error() {
	echo -e "${RED}[ERROR]${NC} $1"
}

# Percentile calculation function
calculate_percentile() {
	local arr=("$@")
	local percentile=$1
	shift
	local arr=("$@")

	# Sort the array
	IFS=$'\n' sorted=($(sort -n <<<"${arr[*]}"))
	unset IFS
	local count=${#sorted[@]}

	if [ $count -eq 0 ]; then
		echo 0
		return
	fi

	local index=$((count * percentile / 100))
	if [ $index -ge $count ]; then
		index=$((count - 1))
	fi

	echo "${sorted[$index]}"
}

# Send command and measure latency
measure_latency() {
	local cmd="$1"
	local node="$2"

	local start=$(date +%s%N)
	local response=$(echo "$cmd" | nc -w 2 $node 2>/dev/null)
	local end=$(date +%s%N)

	local latency=$(((end - start) / 1000000)) # Convert to ms
	echo "$latency"
}

# Send command without measuring (for warmup)
send_command() {
	local cmd="$1"
	local node="$2"

	echo "$cmd" | nc -w 2 $node >/dev/null 2>&1
}

# Compare value against target
check_target() {
	local value=$1
	local target=$2
	local comparison=${3:-"lt"} # lt = less than, gt = greater than

	if [ "$comparison" = "lt" ]; then
		if [ $value -le $target ]; then
			echo "✅"
			return 0
		else
			echo "❌"
			return 1
		fi
	else
		if [ $value -ge $target ]; then
			echo "✅"
			return 0
		else
			echo "❌"
			return 1
		fi
	fi
}

# Create results directory
mkdir -p "$RESULTS_DIR"
log_info "Results directory: $RESULTS_DIR"
log_info "Target node: $TARGET_NODE"
log_info "Secondary node: $SECONDARY_NODE"

echo ""
echo "================================================================================"
echo "DDL CLUSTER BENCHMARK"
echo "================================================================================"
echo ""

# ==============================================================================
# Phase 1: Warmup
# ==============================================================================
log_info "Phase 1: Warmup (5 seconds)"
echo "--------------------------------------------------------------------------------"

warmup_start=$(date +%s)
warmup_count=0

while [ $(($(date +%s) - warmup_count)) -lt 5 ]; do
	# Send light load
	for i in {1..10}; do
		send_command "PING" "$TARGET_NODE"
		send_command "CLAIM warmup_topic_$i" "$TARGET_NODE"
		send_command "GET_OWNER warmup_topic_$i" "$TARGET_NODE"
		send_command "RELEASE warmup_topic_$i" "$TARGET_NODE"
	done
	warmup_count=$((warmup_count + 1))
done

log_success "Warmup complete ($warmup_count iterations)"
sleep 2

# ==============================================================================
# Phase 2: Latency Measurements
# ==============================================================================
echo ""
log_info "Phase 2: Latency Measurements (p50 / p95 / p99)"
echo "--------------------------------------------------------------------------------"

# 2a. Topic Claim Latency
log_info "Measuring topic claim latency (100 samples)..."
for i in {1..100}; do
	latency=$(measure_latency "CLAIM benchmark_topic_$i" "$TARGET_NODE")
	LATENCIES_CLAIM+=("$latency")
done

p50_claim=$(calculate_percentile 50 "${LATENCIES_CLAIM[@]}")
p95_claim=$(calculate_percentile 95 "${LATENCIES_CLAIM[@]}")
p99_claim=$(calculate_percentile 99 "${LATENCIES_CLAIM[@]}")

echo "  Topic Claim:  p50=${p50_claim}ms, p95=${p95_claim}ms, p99=${p99_claim}ms"

# 2b. Topic Release Latency
log_info "Measuring topic release latency (100 samples)..."
for i in {1..100}; do
	# First claim a topic
	send_command "CLAIM benchmark_topic_$i" "$TARGET_NODE"
	# Then measure release
	latency=$(measure_latency "RELEASE benchmark_topic_$i" "$TARGET_NODE")
	LATENCIES_RELEASE+=("$latency")
done

p50_release=$(calculate_percentile 50 "${LATENCIES_RELEASE[@]}")
p95_release=$(calculate_percentile 95 "${LATENCIES_RELEASE[@]}")
p99_release=$(calculate_percentile 99 "${LATENCIES_RELEASE[@]}")

echo "  Topic Release:  p50=${p50_release}ms, p95=${p95_release}ms, p99=${p99_release}ms"

# 2c. Owner Query Latency
log_info "Measuring owner query latency (100 samples)..."
for i in {1..100}; do
	# Claim a topic first
	send_command "CLAIM benchmark_topic_$i" "$TARGET_NODE"
	# Measure query
	latency=$(measure_latency "GET_OWNER benchmark_topic_$i" "$TARGET_NODE")
	LATENCIES_QUERY+=("$latency")
done

p50_query=$(calculate_percentile 50 "${LATENCIES_QUERY[@]}")
p95_query=$(calculate_percentile 95 "${LATENCIES_QUERY[@]}")
p99_query=$(calculate_percentile 99 "${LATENCIES_QUERY[@]}")

echo "  Owner Query:  p50=${p50_query}ms, p95=${p95_query}ms, p99=${p99_query}ms"

# 2d. Lease Acquire Latency
log_info "Measuring lease acquire latency (100 samples)..."
for i in {1..100}; do
	latency=$(measure_latency "ACQUIRE_LEASE benchmark_resource_$i 60" "$TARGET_NODE")
	LATENCIES_LEASE_ACQUIRE+=("$latency")
done

p50_lease_acquire=$(calculate_percentile 50 "${LATENCIES_LEASE_ACQUIRE[@]}")
p95_lease_acquire=$(calculate_percentile 95 "${LATENCIES_LEASE_ACQUIRE[@]}")
p99_lease_acquire=$(calculate_percentile 99 "${LATENCIES_LEASE_ACQUIRE[@]}")

echo "  Lease Acquire:  p50=${p50_lease_acquire}ms, p95=${p95_lease_acquire}ms, p99=${p99_lease_acquire}ms"

# 2e. Lease Release Latency
log_info "Measuring lease release latency (100 samples)..."
for i in {1..100}; do
	# First acquire a lease
	send_command "ACQUIRE_LEASE benchmark_resource_$i 60" "$TARGET_NODE"
	# Then measure release
	latency=$(measure_latency "RELEASE_LEASE benchmark_resource_$i" "$TARGET_NODE")
	LATENCIES_LEASE_RELEASE+=("$latency")
done

p50_lease_release=$(calculate_percentile 50 "${LATENCIES_LEASE_RELEASE[@]}")
p95_lease_release=$(calculate_percentile 95 "${LATENCIES_LEASE_RELEASE[@]}")
p99_lease_release=$(calculate_percentile 99 "${LATENCIES_LEASE_RELEASE[@]}")

echo "  Lease Release:  p50=${p50_lease_release}ms, p95=${p95_lease_release}ms, p99=${p99_lease_release}ms"

log_success "Phase 2 complete"

# ==============================================================================
# Phase 3: Throughput Measurements
# ==============================================================================
echo ""
log_info "Phase 3: Throughput Measurements (ops/sec)"
echo "--------------------------------------------------------------------------------"

# 3a. Topic Claims/sec
log_info "Measuring claim throughput (1000 operations)..."

start_time=$(date +%s%N)
for i in {1..1000}; do
	send_command "CLAIM throughput_topic_$i" "$TARGET_NODE"
done
end_time=$(date +%s%N)

duration_ns=$((end_time - start_time))
duration_ms=$((duration_ns / 1000000))
claims_per_sec=$((1000 * 1000 / duration_ms))

echo "  Topic Claims: $claims_per_sec ops/sec"

# 3b. Queries/sec
log_info "Measuring query throughput (1000 operations)..."

start_time=$(date +%s%N)
for i in {1..1000}; do
	send_command "GET_OWNER throughput_topic_$i" "$TARGET_NODE"
done
end_time=$(date +%s%N)

duration_ns=$((end_time - start_time))
duration_ms=$((duration_ns / 1000000))
queries_per_sec=$((1000 * 1000 / duration_ms))

echo "  Queries: $queries_per_sec ops/sec"

# 3c. Mixed workload (70% reads, 30% writes)
log_info "Measuring mixed workload throughput (1000 operations)..."

start_time=$(date +%s%N)
for i in {1..1000}; do
	if [ $((i % 10)) -lt 7 ]; then
		# 70% reads
		send_command "GET_OWNER throughput_topic_$i" "$TARGET_NODE"
	else
		# 30% writes
		send_command "CLAIM throughput_topic_$i" "$TARGET_NODE"
	fi
done
end_time=$(date +%s%N)

duration_ns=$((end_time - start_time))
duration_ms=$((duration_ns / 1000000))
mixed_per_sec=$((1000 * 1000 / duration_ms))

echo "  Mixed Workload: $mixed_per_sec ops/sec"

log_success "Phase 3 complete"

# ==============================================================================
# Phase 4: Consensus Latency
# ==============================================================================
echo ""
log_info "Phase 4: Consensus Latency (cross-node replication)"
echo "--------------------------------------------------------------------------------"

# 4a. Topic replication latency
log_info "Measuring topic claim replication delay..."

for i in {1..10}; do
	# Claim on primary node
	send_command "CLAIM consensus_topic_$i" "$TARGET_NODE"

	# Immediately query from secondary node
	start=$(date +%s%N)
	response=$(echo "GET_OWNER consensus_topic_$i" | nc -w 2 $SECONDARY_NODE 2>/dev/null)
	end=$(date +%s%N)

	latency=$(((end - start) / 1000000))
	consensus_delays+=("$latency")
done

p99_topic_consensus=$(calculate_percentile 99 "${consensus_delays[@]}")
echo "  Topic replication p99: ${p99_topic_consensus}ms"

# 4b. Lease replication latency
log_info "Measuring lease replication delay..."

for i in {1..10}; do
	# Acquire lease on primary node
	send_command "ACQUIRE_LEASE consensus_resource_$i 60" "$TARGET_NODE"

	# Immediately query from secondary node
	start=$(date +%s%N)
	response=$(echo "GET_LEASE consensus_resource_$i" | nc -w 2 $SECONDARY_NODE 2>/dev/null)
	end=$(date +%s%N)

	latency=$(((end - start) / 1000000))
	lease_consensus_delays+=("$latency")
done

p99_lease_consensus=$(calculate_percentile 99 "${lease_consensus_delays[@]}")
echo "  Lease replication p99: ${p99_lease_consensus}ms"

log_success "Phase 4 complete"

# ==============================================================================
# Phase 5: Results Summary
# ==============================================================================
echo ""
echo "================================================================================"
echo "DDL BENCHMARK RESULTS"
echo "================================================================================"
echo ""

# Latency results
echo "Latency (p50 / p95 / p99):"
echo ""

# Topic Claim
claim_pass=$(check_target $p99_claim $TARGET_LATENCY_CLAIM_P99 "lt")
printf "  %-20s %3dms / %3dms / %3dms  %s (target: < %dms)\n" \
	"Topic Claim:" $p50_claim $p95_claim $p99_claim "$claim_pass" $TARGET_LATENCY_CLAIM_P99

# Topic Release
release_pass=$(check_target $p99_release $TARGET_LATENCY_RELEASE_P99 "lt")
printf "  %-20s %3dms / %3dms / %3dms  %s (target: < %dms)\n" \
	"Topic Release:" $p50_release $p95_release $p99_release "$release_pass" $TARGET_LATENCY_RELEASE_P99

# Owner Query
query_pass=$(check_target $p99_query $TARGET_LATENCY_QUERY_P99 "lt")
printf "  %-20s %3dms / %3dms / %3dms  %s (target: < %dms)\n" \
	"Owner Query:" $p50_query $p95_query $p99_query "$query_pass" $TARGET_LATENCY_QUERY_P99

# Lease Acquire
lease_acquire_pass=$(check_target $p99_lease_acquire $TARGET_LATENCY_LEASE_ACQUIRE_P99 "lt")
printf "  %-20s %3dms / %3dms / %3dms  %s (target: < %dms)\n" \
	"Lease Acquire:" $p50_lease_acquire $p95_lease_acquire $p99_lease_acquire "$lease_acquire_pass" $TARGET_LATENCY_LEASE_ACQUIRE_P99

# Lease Release
lease_release_pass=$(check_target $p99_lease_release $TARGET_LATENCY_LEASE_RELEASE_P99 "lt")
printf "  %-20s %3dms / %3dms / %3dms  %s (target: < %dms)\n" \
	"Lease Release:" $p50_lease_release $p95_lease_release $p99_lease_release "$lease_release_pass" $TARGET_LATENCY_LEASE_RELEASE_P99

echo ""

# Throughput results
echo "Throughput:"
echo ""

# Claims/sec
claims_pass=$(check_target $claims_per_sec $TARGET_THROUGHPUT_CLAIMS "gt")
printf "  %-20s %6d ops/sec  %s (target: > %d)\n" \
	"Topic Claims:" $claims_per_sec "$claims_pass" $TARGET_THROUGHPUT_CLAIMS

# Queries/sec
queries_pass=$(check_target $queries_per_sec $TARGET_THROUGHPUT_QUERIES "gt")
printf "  %-20s %6d ops/sec  %s (target: > %d)\n" \
	"Queries:" $queries_per_sec "$queries_pass" $TARGET_THROUGHPUT_QUERIES

# Mixed workload
mixed_pass=$(check_target $mixed_per_sec $TARGET_THROUGHPUT_MIXED "gt")
printf "  %-20s %6d ops/sec  %s (target: > %d)\n" \
	"Mixed Workload:" $mixed_per_sec "$mixed_pass" $TARGET_THROUGHPUT_MIXED

echo ""

# Consensus latency
echo "Consensus Latency:"
echo ""

topic_consensus_pass=$(check_target $p99_topic_consensus $TARGET_CONSENSUS_LATENCY "lt")
printf "  %-20s %3dms  %s (target: < %dms)\n" \
	"Topic replication:" $p99_topic_consensus "$topic_consensus_pass" $TARGET_CONSENSUS_LATENCY

lease_consensus_pass=$(check_target $p99_lease_consensus $TARGET_CONSENSUS_LATENCY "lt")
printf "  %-20s %3dms  %s (target: < %dms)\n" \
	"Lease replication:" $p99_lease_consensus "$lease_consensus_pass" $TARGET_CONSENSUS_LATENCY

echo ""

# Calculate overall score
score=0
total=10

# Latency scores (5 points)
[ "$claim_pass" = "✅" ] && score=$((score + 1))
[ "$release_pass" = "✅" ] && score=$((score + 1))
[ "$query_pass" = "✅" ] && score=$((score + 1))
[ "$lease_acquire_pass" = "✅" ] && score=$((score + 1))
[ "$lease_release_pass" = "✅" ] && score=$((score + 1))

# Throughput scores (3 points)
[ "$claims_pass" = "✅" ] && score=$((score + 1))
[ "$queries_pass" = "✅" ] && score=$((score + 1))
[ "$mixed_pass" = "✅" ] && score=$((score + 1))

# Consensus scores (2 points)
[ "$topic_consensus_pass" = "✅" ] && score=$((score + 1))
[ "$lease_consensus_pass" = "✅" ] && score=$((score + 1))

# Overall result
if [ $score -ge 8 ]; then
	overall_result="✅"
else
	overall_result="❌"
fi

echo "Overall Score: $score/$total $overall_result"
echo ""
echo "================================================================================"

# Save results to JSON
cat >"$RESULTS_DIR/results.json" <<EOF
{
  "benchmark_timestamp": "$(date -Iseconds)",
  "target_node": "$TARGET_NODE",
  "secondary_node": "$SECONDARY_NODE",
  "latency": {
    "topic_claim": {
      "p50": $p50_claim,
      "p95": $p95_claim,
      "p99": $p99_claim,
      "pass": $([ "$claim_pass" = "✅" ] && echo "true" || echo "false")
    },
    "topic_release": {
      "p50": $p50_release,
      "p95": $p95_release,
      "p99": $p99_release,
      "pass": $([ "$release_pass" = "✅" ] && echo "true" || echo "false")
    },
    "owner_query": {
      "p50": $p50_query,
      "p95": $p95_query,
      "p99": $p99_query,
      "pass": $([ "$query_pass" = "✅" ] && echo "true" || echo "false")
    },
    "lease_acquire": {
      "p50": $p50_lease_acquire,
      "p95": $p95_lease_acquire,
      "p99": $p99_lease_acquire,
      "pass": $([ "$lease_acquire_pass" = "✅" ] && echo "true" || echo "false")
    },
    "lease_release": {
      "p50": $p50_lease_release,
      "p95": $p95_lease_release,
      "p99": $p99_lease_release,
      "pass": $([ "$lease_release_pass" = "✅" ] && echo "true" || echo "false")
    }
  },
  "throughput": {
    "topic_claims_per_sec": $claims_per_sec,
    "queries_per_sec": $queries_per_sec,
    "mixed_workload_per_sec": $mixed_per_sec,
    "claims_pass": $([ "$claims_pass" = "✅" ] && echo "true" || echo "false"),
    "queries_pass": $([ "$queries_pass" = "✅" ] && echo "true" || echo "false"),
    "mixed_pass": $([ "$mixed_pass" = "✅" ] && echo "true" || echo "false")
  },
  "consensus": {
    "topic_replication_p99": $p99_topic_consensus,
    "lease_replication_p99": $p99_lease_consensus,
    "topic_pass": $([ "$topic_consensus_pass" = "✅" ] && echo "true" || echo "false"),
    "lease_pass": $([ "$lease_consensus_pass" = "✅" ] && echo "true" || echo "false")
  },
  "overall_score": $score,
  "total_score": $total,
  "overall_pass": $([ "$overall_result" = "✅" ] && echo "true" || echo "false")
}
EOF

log_success "Results saved to: $RESULTS_DIR/results.json"

# Cleanup benchmark topics
log_info "Cleaning up benchmark topics..."
for i in {1..100}; do
	send_command "RELEASE benchmark_topic_$i" "$TARGET_NODE" 2>/dev/null || true
	send_command "RELEASE throughput_topic_$i" "$TARGET_NODE" 2>/dev/null || true
	send_command "RELEASE consensus_topic_$i" "$TARGET_NODE" 2>/dev/null || true
	send_command "RELEASE_LEASE benchmark_resource_$i" "$TARGET_NODE" 2>/dev/null || true
	send_command "RELEASE_LEASE consensus_resource_$i" "$TARGET_NODE" 2>/dev/null || true
done

log_success "Benchmark complete!"

exit 0
