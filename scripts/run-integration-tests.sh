#!/bin/bash
#
# Run integration benchmarks against a live cluster
#
# This script assumes a cluster is already running (start with start-cluster.sh)
#

set -e

BASE_PORT=${1:-7000}
NUM_NODES=${2:-3}

echo "Running integration benchmarks against cluster (base port: $BASE_PORT, nodes: $NUM_NODES)"

# Set environment variables for the benchmark
export RUST_TEST_NODES="localhost:[${BASE_PORT}-$((BASE_PORT + NUM_NODES - 1))]"
export RUST_TEST_DURATION="60"

echo "Node specification: $RUST_TEST_NODES"
echo "Duration: ${RUST_TEST_DURATION}s"
echo ""

# Run benchmarks
echo "Running full benchmark suite..."
cargo test --release --test distributed_benchmark --features integration-tests -- \
	--nocapture full_benchmark_suite

echo ""
echo "Benchmark complete!"
