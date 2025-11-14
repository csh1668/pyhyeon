#!/usr/bin/env python3
"""
Detailed JIT Performance Benchmark

Tests with different iteration counts to measure:
1. Startup overhead
2. JIT compilation benefit
3. Net performance gain
"""

import subprocess
import time

def run_test(test_file, name):
    times = []
    for i in range(5):
        start = time.perf_counter()
        result = subprocess.run(
            ['target\\release\\pyhc.exe', 'run', test_file],
            capture_output=True,
            text=True,
            shell=True
        )
        end = time.perf_counter()

        if result.returncode == 0:
            elapsed = (end - start) * 1000
            times.append(elapsed)
        else:
            print(f"ERROR: {result.stderr[:100]}")
            return None

    avg = sum(times) / len(times)
    return avg

print("=" * 80)
print("JIT Performance Benchmark - Detailed Analysis")
print("=" * 80)
print("\nJIT Configuration:")
print("  Threshold: 1000 executions")
print("  After 1000 calls: function is compiled to native x86-64 code")
print("=" * 80)

# Test with different iteration counts
tests = [
    ("tests/programs/benchmark_arithmetic.pyh", "10K iterations", 10000),
    ("tests/programs/benchmark_long.pyh", "100K iterations", 100000),
]

results = []
for test_file, name, iterations in tests:
    print(f"\n{name}:")
    print("-" * 80)
    avg = run_test(test_file, name)
    if avg:
        results.append((name, iterations, avg))
        print(f"  Average time: {avg:.2f}ms")

        # Calculate effective time per iteration
        time_per_iter = avg / iterations * 1000  # microseconds
        print(f"  Per iteration: {time_per_iter:.4f}μs")

        # Estimate JIT vs Interpreter split
        if iterations > 1000:
            interp_iters = 1000
            jit_iters = iterations - 1000
            print(f"  Interpreter: {interp_iters} iterations")
            print(f"  JIT native:  {jit_iters} iterations ({jit_iters/iterations*100:.1f}%)")

print("\n" + "=" * 80)
print("Analysis:")
print("=" * 80)

if len(results) >= 2:
    # Compare 10K vs 100K to see scaling
    name1, iters1, time1 = results[0]
    name2, iters2, time2 = results[1]

    per_iter1 = time1 / iters1 * 1000  # μs
    per_iter2 = time2 / iters2 * 1000  # μs

    print(f"\n10K iterations:  {per_iter1:.4f}μs per iteration")
    print(f"100K iterations: {per_iter2:.4f}μs per iteration")

    if per_iter1 > per_iter2:
        improvement = ((per_iter1 - per_iter2) / per_iter1) * 100
        print(f"\nSpeedup with more JIT usage: {improvement:.1f}%")
        print(f"This shows JIT is working! Longer runs benefit more from native code.")
    else:
        print(f"\nNo improvement detected. This suggests:")
        print("  1. JIT compilation overhead dominates for this workload")
        print("  2. Or the test is too simple to benefit from JIT")

print("\n" + "=" * 80)
print("Note: For accurate measurement, the 100K test should show lower")
print("per-iteration time due to higher percentage of JIT-compiled code.")
print("=" * 80)
