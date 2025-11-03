# Pyhyeon Web Playground

웹 브라우저에서 Pyhyeon 코드를 실행할 수 있는 WASM 기반 플레이그라운드입니다.

## 기능

- ✅ Monaco Editor 기반 코드 편집기
- ✅ 실시간 문법 검사 (syntax highlighting)
- ✅ 에러 표시 및 마커
- ✅ WASM VM을 통한 코드 실행
- ✅ 입출력 지원 (`input()`, `print()`)
- ✅ 40개 이상의 예제 프로그램
- ✅ List, Dict, For 루프 완전 지원

## 빌드 및 실행

### WASM 빌드
```bash
# wasm-pack 설치 (최초 1회)
cargo install wasm-pack

# WASM 빌드
pnpm wasm
```

### 개발 서버 실행
```bash
# 의존성 설치
pnpm install

# 개발 서버 시작
pnpm dev
```

### 프로덕션 빌드
```bash
pnpm build
```

## 구조

```
web/
├── src/
│   ├── App.tsx           # 메인 앱 컴포넌트
│   ├── components/ui/    # UI 컴포넌트
│   └── lib/utils.ts      # 유틸리티 함수
├── public/
│   └── examples/         # 예제 프로그램들
│       ├── examples.json # 예제 목록
│       └── *.pyh         # 예제 파일들
└── pkg/                  # WASM 빌드 결과
```

## 기술 스택

- **Frontend**: React + TypeScript + Vite
- **Editor**: Monaco Editor (VS Code 편집기)
- **VM**: Rust → WASM (wasm-pack)
- **UI**: shadcn/ui + Tailwind CSS

## 예제 카테고리

- **Collections**: List, Dict 기본/메서드/for 루프
- **Loops**: range, for 루프 변형
- **String**: 문자열 기본/고급/메서드
- **Functions**: 재귀, 반복
- **Class**: 클래스 정의 및 사용
- **Input/Output**: input() 다양한 사용법
- **Conditionals**: if/elif/else 분기문
