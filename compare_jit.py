#!/usr/bin/env python3
"""
JIT vs Interpreter 성능 비교

현재 JIT 구현의 한계:
- JIT는 재귀 함수 호출(Call 명령어)을 지원하지 않음
- 지원되는 명령어: 상수, 로컬 변수, 산술(+,-,*), 비교(==,<), 제어 흐름
- 지원되지 않는 명령어는 인터프리터로 폴백

따라서 순수 산술 연산이나 간단한 함수만 JIT 컴파일 가능
"""

import subprocess
import time

def run_test(test_file, description):
    print(f"\n{'='*60}")
    print(f"{description}")
    print('='*60)

    times = []
    for i in range(5):
        start = time.time()
        result = subprocess.run(
            ['target\\release\\pyhc.exe', 'run', test_file],
            capture_output=True,
            text=True,
            shell=True
        )
        end = time.time()

        if result.returncode == 0:
            elapsed = (end - start) * 1000
            times.append(elapsed)
            print(f"  Run {i+1}: {elapsed:.2f}ms")
        else:
            print(f"  ERROR: {result.stderr[:100]}")
            return None

    if times:
        avg = sum(times) / len(times)
        print(f"\n  Average: {avg:.2f}ms")
        print(f"  Min:     {min(times):.2f}ms")
        print(f"  Max:     {max(times):.2f}ms")
        return avg
    return None

print("=" * 80)
print(" " * 20 + "Pyhyeon JIT Performance Analysis")
print("=" * 80)

print("\n현재 JIT 상태:")
print("  - JIT 임계값: 1000회 실행")
print("  - 지원 명령어: 상수, 로컬변수, 산술(+,-,*), 비교(==,<), 분기")
print("  - 미지원: 함수 호출(Call), 메서드 호출, 컬렉션 등")

# Test 1: 산술 연산 (JIT 지원)
avg1 = run_test(
    'tests/programs/benchmark_arithmetic.pyh',
    "Test 1: Pure Arithmetic (10,000 iterations)\n" +
    "  - Pure arithmetic operations\n" +
    "  - JIT compilation: YES"
)

# Test 2: 간단한 함수 (JIT 지원)
avg2 = run_test(
    'tests/programs/jit_simple.pyh',
    "Test 2: Simple Function (1,100 calls)\n" +
    "  - Simple add(a, b) function\n" +
    "  - JIT compilation: YES"
)

print("\n" + "=" * 80)
print("분석:")
print("=" * 80)
print("\n1. JIT가 동작하는 케이스:")
print("   - 처음 1000번은 인터프리터 모드 실행")
print("   - 1001번째부터는 JIT 컴파일된 네이티브 코드 실행")
print("   - 위 테스트에서 후반부는 JIT 네이티브 코드로 실행됨")

print("\n2. 성능 특성:")
print("   - 인터프리터: 매 명령어마다 match/switch 오버헤드")
print("   - JIT: 직접 x86-64 네이티브 코드 실행")
print("   - 예상 속도 향상: 5-10배 (hot path에 대해)")

print("\n3. 현재 한계:")
print("   - 재귀 함수: JIT 불가 (Call 명령어 미지원)")
print("   - 복잡한 연산: JIT 불가 (많은 명령어 미지원)")
print("   - 위 한계는 향후 확장으로 개선 가능")

print("\n" + "=" * 80)
