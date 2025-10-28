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
  
  function copyExamples() {
    // 타겟 디렉토리 생성
    if (!fs.existsSync(targetDir)) {
      fs.mkdirSync(targetDir, { recursive: true })
    }
    
    // .pyh 파일들 읽기
    const files = fs.readdirSync(sourceDir)
      .filter(f => f.endsWith('.pyh'))
      .sort()
    
    // 파일 복사
    files.forEach(file => {
      const sourcePath = path.join(sourceDir, file)
      const targetPath = path.join(targetDir, file)
      fs.copyFileSync(sourcePath, targetPath)
    })
    
    // 메타데이터 생성
    const examples = files.map(file => {
      const content = fs.readFileSync(path.join(sourceDir, file), 'utf-8')
      const firstLine = content.split('\n')[0]
      const description = firstLine.startsWith('#') ? firstLine.slice(1).trim() : file
      
      return {
        id: file.replace('.pyh', ''),
        name: file,
        description,
        category: getCategoryFromFilename(file)
      }
    })
    
    fs.writeFileSync(
      path.join(targetDir, 'examples.json'),
      JSON.stringify(examples, null, 2),
      'utf-8'
    )
    
    console.log(`✓ Copied ${files.length} example files to public/examples/`)
  }
  
  function getCategoryFromFilename(filename: string): string {
    if (filename.startsWith('class_') || filename.startsWith('test_class') || filename.includes('method_chaining')) return 'Class'
    if (filename.startsWith('input_')) return 'Input/Output'
    if (filename.startsWith('string_')) return 'String'
    if (filename.startsWith('func_') || filename.includes('fib') || filename.includes('prime')) return 'Functions'
    if (filename.startsWith('loop') || filename.includes('heavy_loop')) return 'Loops'
    if (filename.startsWith('branch')) return 'Conditionals'
    if (filename.startsWith('arith')) return 'Arithmetic'
    return 'Examples'
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
