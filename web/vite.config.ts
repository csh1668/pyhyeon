import path from "path"
import fs from "fs"
import tailwindcss from "@tailwindcss/vite"
import { defineConfig } from 'vite'
import type { Plugin } from 'vite'
import react from '@vitejs/plugin-react-swc'

// 예제 파일 복사 플러그인
function copyExamplesPlugin(): Plugin {
  const sourceDir = path.resolve(__dirname, '../tests/programs')
  const targetDir = path.resolve(__dirname, './public/examples')

  function getCategoryFromPath(folderName: string, filename: string): string {
    // 폴더 구조 기반 카테고리 매핑
    const categoryMap: Record<string, string> = {
      'basics': 'Basics',
      'loops': 'Loops',
      'collections': 'Collections',
      'classes': 'Classes',
      'io': 'Input/Output',
      'strings': 'Strings'
    }

    // 폴더명이 있으면 그걸 사용, 없으면 루트 디렉토리로 간주
    if (folderName && folderName !== '.') {
      return categoryMap[folderName] || folderName.charAt(0).toUpperCase() + folderName.slice(1)
    }

    // 하위 호환성: 폴더 없는 파일들을 위한 기존 로직
    if (filename.startsWith('class_') || filename.startsWith('test_class') || filename.includes('method_chaining')) return 'Classes'
    if (filename.startsWith('input_')) return 'Input/Output'
    if (filename.startsWith('string_')) return 'Strings'
    if (filename.startsWith('func_') || filename.includes('fib') || filename.includes('prime')) return 'Functions'
    if (filename.startsWith('loop') || filename.includes('heavy_loop')) return 'Loops'
    if (filename.startsWith('branch')) return 'Conditionals'
    if (filename.startsWith('arith')) return 'Arithmetic'
    return 'Examples'
  }

  function copyExamples() {
    // 타겟 디렉토리 생성
    if (!fs.existsSync(targetDir)) {
      fs.mkdirSync(targetDir, { recursive: true })
    }

    // 재귀적으로 .pyh 파일들 찾기
    function findPyhFiles(dir: string, baseDir: string = dir): Array<{ relativePath: string, fullPath: string }> {
      const results: Array<{ relativePath: string, fullPath: string }> = []
      const entries = fs.readdirSync(dir, { withFileTypes: true })

      for (const entry of entries) {
        const fullPath = path.join(dir, entry.name)
        if (entry.isDirectory()) {
          results.push(...findPyhFiles(fullPath, baseDir))
        } else if (entry.isFile() && entry.name.endsWith('.pyh')) {
          const relativePath = path.relative(baseDir, fullPath)
          results.push({ relativePath, fullPath })
        }
      }

      return results
    }

    const pyhFiles = findPyhFiles(sourceDir).sort((a, b) => a.relativePath.localeCompare(b.relativePath))

    // 파일 복사 (폴더 구조 유지)
    pyhFiles.forEach(({ relativePath, fullPath }) => {
      const targetPath = path.join(targetDir, relativePath)
      const targetDirPath = path.dirname(targetPath)

      // 중첩 폴더 생성
      if (!fs.existsSync(targetDirPath)) {
        fs.mkdirSync(targetDirPath, { recursive: true })
      }

      fs.copyFileSync(fullPath, targetPath)
    })

    // 메타데이터 생성
    const examples = pyhFiles.map(({ relativePath, fullPath }) => {
      const content = fs.readFileSync(fullPath, 'utf-8')
      const firstLine = content.split('\n')[0]
      const description = firstLine.startsWith('#') ? firstLine.slice(1).trim() : path.basename(relativePath)

      // 폴더명을 카테고리로 사용 (basics/arith.pyh -> Basic)
      const folderName = path.dirname(relativePath)
      const category = getCategoryFromPath(folderName, path.basename(relativePath))

      return {
        id: relativePath.replace('.pyh', '').replace(/\\/g, '/'),
        name: path.basename(relativePath),
        path: relativePath.replace(/\\/g, '/'),
        description,
        category
      }
    })

    fs.writeFileSync(
      path.join(targetDir, 'examples.json'),
      JSON.stringify(examples, null, 2),
      'utf-8'
    )

    console.log(`✓ Copied ${pyhFiles.length} example files to public/examples/`)
  }
  
  return {
    name: 'copy-examples',
    buildStart() {
      copyExamples()
    },
    configureServer(server) {
      // dev 서버 시작 시에도 복사
      copyExamples()
      
      // 소스 파일 변경 감지
      server.watcher.add(sourceDir)
      server.watcher.on('change', (file) => {
        if (file.startsWith(sourceDir) && file.endsWith('.pyh')) {
          copyExamples()
          server.ws.send({ type: 'full-reload' })
        }
      })
    }
  }
}

// https://vite.dev/config/
export default defineConfig({
  base: '/pyhyeon/',
  plugins: [react(), tailwindcss(), copyExamplesPlugin()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "@pkg": path.resolve(__dirname, "./pkg"),
    },
  },
})
