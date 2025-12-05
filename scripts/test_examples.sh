#!/bin/bash

echo "=== TESTING ALL WORKING EXAMPLES ==="

examples=(
    "engine_demonstration"
    "engine_trait_demo" 
    "pubsub_demo"
    "expression_engine_demo"
    "health_score_demo"
    "queue_engine_integration"
    "helper_methods_demo"
    "simple_engine_demo"
    "engine_integration"
    "simple_engine_usage"
    "real_system_monitoring"
    "queue_as_engine"
)

success_count=0
fail_count=0

for example in "${examples[@]}"; do
    echo ""
    echo "Testing: $example"
    
    timeout 10s cargo run --example "$example" > /tmp/output.log 2>&1
    if [ $? -eq 0 ]; then
        echo "$example: SUCCESS"
        head -3 /tmp/output.log
        ((success_count++))
    else
        echo "$example: FAILED/TIMED OUT"
        head -5 /tmp/output.log
        ((fail_count++))
    fi
    
    sleep 1
done

echo ""
echo "=== SUMMARY ==="
echo "Successful: $success_count"
echo "Failed/Timeout: $fail_count"
echo "Total: ${#examples[@]}"

if [ $fail_count -eq 0 ]; then
    echo "ALL EXAMPLES RUNNING!"
else
    echo "Some examples had issues"
fi
    
    sleep 1
done

echo ""
echo "=== SUMMARY ==="
echo "✅ Successful: $success_count"
echo "❌ Failed/Timeout: $fail_count"
echo "📊 Total: ${#examples[@]}"

if [ $fail_count -eq 0 ]; then
    echo "🎉 ALL EXAMPLES RUNNING! 🎉"
else
    echo "⚠️  Some examples had issues"
fi
