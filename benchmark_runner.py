#!/usr/bin/env python3
import subprocess
import time

def run_benchmark(test_file, iterations=3):
    """Run benchmark multiple times and return average execution time"""
    times = []

    for i in range(iterations):
        start = time.time()
        result = subprocess.run(
            ['target\\release\\pyhc.exe', 'run', test_file],
            capture_output=True,
            text=True,
            shell=True
        )
        end = time.time()

        if result.returncode == 0:
            elapsed = (end - start) * 1000  # Convert to milliseconds
            times.append(elapsed)
            print(f"  Run {i+1}: {elapsed:.2f}ms")
        else:
            print(f"  Run {i+1}: ERROR")
            print(result.stderr)

    if times:
        avg = sum(times) / len(times)
        min_time = min(times)
        max_time = max(times)
        return avg, min_time, max_time
    return None, None, None

print("=" * 60)
print("Pyhyeon JIT Performance Benchmark")
print("=" * 60)

# Benchmark 1: Arithmetic (10,000 iterations)
print("\nBenchmark: Pure Arithmetic (10,000 iterations)")
print("- Operations: add, mul, sub in a loop")
print("- Expected: JIT kicks in after 1000 iterations")
print("-" * 60)
avg, min_t, max_t = run_benchmark('tests/programs/benchmark_arithmetic.pyh', iterations=3)
if avg:
    print(f"\nResults:")
    print(f"  Average: {avg:.2f}ms")
    print(f"  Min:     {min_t:.2f}ms")
    print(f"  Max:     {max_t:.2f}ms")

# Benchmark 2: Simple function call (1100 iterations)
print("\n" + "=" * 60)
print("Benchmark: Simple Function (1100 calls)")
print("- Function: add(10, 20)")
print("- Expected: JIT compiles after 1000 calls")
print("-" * 60)
avg, min_t, max_t = run_benchmark('tests/programs/jit_simple.pyh', iterations=3)
if avg:
    print(f"\nResults:")
    print(f"  Average: {avg:.2f}ms")
    print(f"  Min:     {min_t:.2f}ms")
    print(f"  Max:     {max_t:.2f}ms")

print("\n" + "=" * 60)
print("Note: JIT compilation happens at runtime after 1000 executions.")
print("The first 1000 iterations run in interpreter mode, then switch to")
print("native code execution for the remaining iterations.")
print("=" * 60)
