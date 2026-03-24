#!/bin/bash
# run_benchmarks.sh

echo "Running DDL Benchmarks..."
echo "========================"

cargo bench --bench ddl_benchmarks

echo ""
echo "Results saved to target/criterion/"
echo "View with: cargo install cargo-criterion && cargo criterion"