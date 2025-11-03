import { useEffect, useRef, useState, useCallback, useMemo } from 'react'
import * as monaco from 'monaco-editor'
import { Github, Play, Loader2, StopCircle, Send, Terminal, Code2, FileCode } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { initWasm } from './lib/utils'
import { 
  analyze as wasmAnalyze,
  start_program,
  provide_input,
  stop_program
} from '@pkg/pyhyeon'
import { AnsiUp } from 'ansi_up'

type VmState = 'idle' | 'running' | 'waiting_for_input' | 'finished' | 'error'

interface VmStateInfo {
  state: string
  output: string
  execution_time_ms?: number
}

interface Example {
  id: string
  name: string
  description: string
  category: string
}

function App() {
  const editorRef = useRef<HTMLDivElement>(null)
  const [editor, setEditor] = useState<monaco.editor.IStandaloneCodeEditor | null>(null)
  const [output, setOutput] = useState<string>("")
  const analyzeTimeoutRef = useRef<number | null>(null)
  const [editorWidth, setEditorWidth] = useState<number>(60) // 60% default
  const [isResizing, setIsResizing] = useState(false)
  const [wasmReady, setWasmReady] = useState(false)
  const [vmState, setVmState] = useState<VmState>('idle')
  const [inputValue, setInputValue] = useState<string>('')
  const inputRef = useRef<HTMLInputElement>(null)
  const outputEndRef = useRef<HTMLDivElement>(null)
  const [examples, setExamples] = useState<Example[]>([])
  const [selectedExample, setSelectedExample] = useState<string>('')
  const [executionTime, setExecutionTime] = useState<number | null>(null)
  
  // ANSI to HTML converter
  const ansiUp = useMemo(() => new AnsiUp(), [])
  
  // Convert ANSI output to HTML
  const outputHtml = useMemo(() => {
    return ansiUp.ansi_to_html(output)
  }, [output, ansiUp])

  useEffect(() => {
    const init = async () => {
      try {
        await initWasm()
        setWasmReady(true)
      } catch (error) {
        console.error('WASM init error:', error)
      }
    }
    init()
  }, [])

  // 예제 파일 목록 로드
  useEffect(() => {
    const loadExamples = async () => {
      try {
        const response = await fetch('/pyhyeon/examples/examples.json')
        if (response.ok) {
          const data = await response.json()
          setExamples(data)
        }
      } catch (error) {
        console.error('Failed to load examples:', error)
      }
    }
    loadExamples()
  }, [])

  // 선택된 예제 파일 로드
  const loadExample = useCallback(async (exampleId: string) => {
    if (!editor || !exampleId) return
    
    try {
      const example = examples.find(ex => ex.id === exampleId)
      if (!example) return
      
      const response = await fetch(`/pyhyeon/examples/${example.name}`)
      if (response.ok) {
        const code = await response.text()
        editor.setValue(code)
        setSelectedExample(exampleId)
        setOutput('')
        setVmState('idle')
        setExecutionTime(null)
      }
    } catch (error) {
      console.error('Failed to load example:', error)
    }
  }, [editor, examples])

  // Handle resize
  const handleMouseDown = useCallback(() => {
    setIsResizing(true)
  }, [])

  useEffect(() => {
    if (!isResizing) return

    const handleMouseMove = (e: MouseEvent) => {
      const newWidth = (e.clientX / window.innerWidth) * 100
      // Limit between 30% and 80%
      if (newWidth >= 30 && newWidth <= 80) {
        setEditorWidth(newWidth)
      }
    }

    const handleMouseUp = () => {
      setIsResizing(false)
    }

    document.addEventListener('mousemove', handleMouseMove)
    document.addEventListener('mouseup', handleMouseUp)

    return () => {
      document.removeEventListener('mousemove', handleMouseMove)
      document.removeEventListener('mouseup', handleMouseUp)
    }
  }, [isResizing])

  // Auto-analyze function with debounce
  const autoAnalyze = useCallback((editorInstance: monaco.editor.IStandaloneCodeEditor) => {
    if (!wasmReady) return
    
    const model = editorInstance.getModel()
    if (!model) return
    let src = model.getValue()
    if (!src.endsWith('\n')) {
      src += '\n'
    }

    try {
      const diags: any[] = (wasmAnalyze as any)(src) || []
      const markers = diags.map((d: any) => ({
        message: d.message,
        startLineNumber: (d.start_line ?? 0) + 1,
        startColumn: (d.start_char ?? 0) + 1,
        endLineNumber: (d.end_line ?? 0) + 1,
        endColumn: (d.end_char ?? 0) + 1,
        severity: monaco.MarkerSeverity.Error,
      }))
      monaco.editor.setModelMarkers(model, 'pyh', markers)
    } catch (e) {
      console.error('Analyze error:', e)
    }
  }, [wasmReady])

  useEffect(() => {
    let instance: monaco.editor.IStandaloneCodeEditor | null = null
    let disposeChange: monaco.IDisposable | null = null

    async function init() {
      // 커스텀 다크 테마 정의
      monaco.editor.defineTheme('pyhyeon-dark', {
        base: 'vs-dark',
        inherit: true,
        rules: [
          // 주석 - 녹색, 이탤릭
          { token: 'comment', foreground: '6a9955', fontStyle: 'italic' },
          
          // 키워드 - 보라색
          { token: 'keyword', foreground: 'c586c0', fontStyle: 'bold' },
          
          // 상수 (True, False, None) - 파란색
          { token: 'constant', foreground: '569cd6', fontStyle: 'bold' },
          
          // 내장 함수 - 노란색
          { token: 'builtin', foreground: 'dcdcaa' },
          
          // 특별한 식별자 (self) - 밝은 파란색
          { token: 'special', foreground: '4fc1ff', fontStyle: 'italic' },
          
          // 클래스 이름 - 밝은 녹색
          { token: 'type.identifier', foreground: '4ec9b0', fontStyle: 'bold' },
          
          // 매직 메서드 (__init__, __str__ 등) - 자주색
          { token: 'magic', foreground: 'c586c0' },
          
          // 숫자 - 연한 녹색
          { token: 'number', foreground: 'b5cea8' },
          
          // 문자열 - 주황색
          { token: 'string', foreground: 'ce9178' },
          { token: 'string.escape', foreground: 'd7ba7d' },
          { token: 'string.invalid', foreground: 'f44747' },
          
          // 연산자 - 흰색
          { token: 'operator', foreground: 'd4d4d4' },
          
          // 구분자 (괄호, 콜론 등) - 밝은 회색
          { token: 'delimiter', foreground: 'd4d4d4' },
          { token: 'delimiter.parenthesis', foreground: 'ffd700' },
          
          // 식별자 - 하늘색
          { token: 'identifier', foreground: '9cdcfe' },
        ],
        colors: {
          'editor.background': '#00000000', // 완전 투명
          'editor.foreground': '#fafafa',
          'editor.lineHighlightBackground': '#ffffff08',
          'editorLineNumber.foreground': '#4a4a4a',
          'editorLineNumber.activeForeground': '#9a9a9a',
          'editor.selectionBackground': '#2a2a2a',
          'editor.inactiveSelectionBackground': '#1a1a1a',
          'editorCursor.foreground': '#fafafa',
          'editorWhitespace.foreground': '#2a2a2a',
          'editorIndentGuide.background': '#2a2a2a',
          'editorIndentGuide.activeBackground': '#3a3a3a',
          'editor.selectionHighlightBackground': '#14141420',
          'editor.wordHighlightBackground': '#14141420',
          'editorBracketMatch.background': '#14141440',
          'editorBracketMatch.border': '#4a4a4a',
        },
      })

      // 언어 등록 (Monarch tokenizer)
      monaco.languages.register({ id: 'pyh' })
      monaco.languages.setMonarchTokensProvider('pyh', {
        // 현재 pyhyeon에 구현된 키워드들
        keywords: [
          'if', 'elif', 'else', 'while', 'for', 'in', 'def', 'return', 
          'and', 'or', 'not', 'class'
        ],
        
        // 상수 키워드
        constants: ['None', 'True', 'False'],
        
        // 특별한 식별자
        special: ['self'],
        
        // 내장 함수들
        builtins: ['print', 'input', 'int', 'bool', 'str', 'len', 'range', 'list', 'dict'],
        
        // 연산자들
        operators: [
          '=', '==', '!=', '<', '<=', '>', '>=',
          '+', '-', '*', '//', '%'
        ],
        
        // 구분자들
        delimiters: ['(', ')', '[', ']', '{', '}', ':', ',', ';', '.'],
        
        tokenizer: {
          root: [
            // 주석 (# 으로 시작)
            [/#.*$/, 'comment'],
            
            // 문자열 리터럴 (큰따옴표, 작은따옴표)
            [/"([^"\\]|\\.)*$/, 'string.invalid'],  // 닫히지 않은 문자열
            [/'([^'\\]|\\.)*$/, 'string.invalid'],  // 닫히지 않은 문자열
            [/"/, 'string', '@string_double'],
            [/'/, 'string', '@string_single'],
            
            // 숫자 (정수만 지원)
            [/\d+/, 'number'],
            
            // 매직 메서드 (__init__, __str__ 등)
            [/__[a-zA-Z_]\w*__/, 'magic'],
            
            // class 키워드 - 상태 전환으로 클래스 이름 특별 처리
            [/class(?=\s)/, { token: 'keyword', next: '@className' }],
            
            // 키워드, 상수, 특별한 식별자, 내장함수, 일반 식별자
            [/[a-zA-Z_]\w*/, {
              cases: {
                '@keywords': 'keyword',
                '@constants': 'constant',
                '@special': 'special',
                '@builtins': 'builtin',
                '@default': 'identifier'
              }
            }],
            
            // 연산자
            [/==|!=|<=|>=|\/\/|[+\-*%<>=]/, 'operator'],
            
            // 구분자
            [/[()\[\]{}\:,;.]/, 'delimiter'],
            
            // 괄호 강조
            [/[\[\]]/, 'delimiter.bracket'],
            [/[{}]/, 'delimiter.brace'],
            
            // 공백
            [/[ \t\r\n]+/, 'white'],
          ],
          
          // 클래스 이름 상태 (class 키워드 다음)
          className: [
            [/[ \t\r\n]+/, 'white'],
            [/[a-zA-Z_]\w*/, { token: 'type.identifier', next: '@pop' }],
            [/./, { token: '@rematch', next: '@pop' }],
          ],
          
          // 큰따옴표 문자열 처리
          string_double: [
            [/[^\\"]+/, 'string'],
            [/\\./, 'string.escape'],
            [/"/, 'string', '@pop'],
          ],
          
          // 작은따옴표 문자열 처리
          string_single: [
            [/[^\\']+/, 'string'],
            [/\\./, 'string.escape'],
            [/'/, 'string', '@pop'],
          ],
        },
      } as any)

      // 괄호 매칭 설정
      monaco.languages.setLanguageConfiguration('pyh', {
        brackets: [
          ['(', ')'],
          ['[', ']'],
          ['{', '}'],
        ],
        autoClosingPairs: [
          { open: '(', close: ')' },
          { open: '[', close: ']' },
          { open: '{', close: '}' },
          { open: '"', close: '"' },
          { open: "'", close: "'" },
        ],
        surroundingPairs: [
          { open: '(', close: ')' },
          { open: '[', close: ']' },
          { open: '{', close: '}' },
          { open: '"', close: '"' },
          { open: "'", close: "'" },
        ],
        comments: {
          lineComment: '#',
        },
        onEnterRules: [
          {
            // 콜론(:)으로 끝나는 라인에서 엔터를 누르면 2칸 들여쓰기
            beforeText: /^\s*.*:\s*$/,
            action: { indentAction: monaco.languages.IndentAction.Indent }
          },
          {
            // 일반 라인에서 엔터를 누르면 현재 들여쓰기 유지
            beforeText: /.+$/,
            action: { indentAction: monaco.languages.IndentAction.None }
          }
        ],
      })

      instance = monaco.editor.create(editorRef.current!, {
        value: '# Welcome to Pyhyeon!\n\n# Lists\nnums = [1, 2, 3, 4, 5]\nfor n in nums:\n  print(n)\n\n# Dicts\nperson = {"name": "Alice", "age": 30}\nprint(person["name"])\n',
        language: 'pyh',
        theme: 'pyhyeon-dark',
        automaticLayout: true,
        minimap: { enabled: false },
        fontSize: 14,
        lineHeight: 24,
        fontFamily: "'Monaco', 'Menlo', 'Ubuntu Mono', monospace",
        padding: { top: 50, bottom: 16 },
        scrollBeyondLastLine: false,
        renderLineHighlight: 'all',
        cursorBlinking: 'smooth',
        cursorSmoothCaretAnimation: 'on',
        smoothScrolling: true,
        
        // 들여쓰기 설정 (2칸)
        tabSize: 2,
        insertSpaces: true,
        detectIndentation: false,
        
        // 향상된 편집 기능
        bracketPairColorization: {
          enabled: true,
        },
        matchBrackets: 'always',
        autoClosingBrackets: 'always',
        autoClosingQuotes: 'always',
        autoIndent: 'full',
        formatOnType: true,
        formatOnPaste: true,
        
        // 제안 및 힌트 기능 (문서 내 단어 기반 자동완성만 활성화)
        quickSuggestions: {
          other: true,
          comments: false,
          strings: false,
        },
        suggestOnTriggerCharacters: false,
        acceptSuggestionOnCommitCharacter: true,
        acceptSuggestionOnEnter: 'on',
        tabCompletion: 'on',
        wordBasedSuggestions: 'currentDocument',  // 현재 문서의 단어 기반 자동완성
        
        // 호버 기능 (에러 설명 표시)
        hover: {
          enabled: true,
          delay: 300,
          sticky: true,
        },
        
        // 파라미터 힌트 비활성화
        parameterHints: {
          enabled: false,
        },
        
        // 기타 UI 개선
        folding: true,
        foldingStrategy: 'indentation',
        showFoldingControls: 'mouseover',
        occurrencesHighlight: 'singleFile',
        selectionHighlight: true,
        renderWhitespace: 'selection',
      })
      setEditor(instance)

      // Add keyboard shortcut for Run (Ctrl+Enter)
      instance.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, () => {
        if (!wasmReady) {
          return
        }
        
        const code = instance!.getValue()
        setOutput('')
        setVmState('running')
        setExecutionTime(null)
        
        try {
          const result = start_program(code) as VmStateInfo
          
          setOutput(result.output)
          setVmState(result.state as VmState)
          if (result.execution_time_ms !== undefined) {
            setExecutionTime(result.execution_time_ms)
          }
        } catch (e) {
          setOutput(`${e}`)
          setVmState('error')
          console.error('Run error:', e)
        }
      })

      // Auto-analyze on content change with debounce
      disposeChange = instance.onDidChangeModelContent(() => {
        if (analyzeTimeoutRef.current) {
          clearTimeout(analyzeTimeoutRef.current)
        }
        analyzeTimeoutRef.current = setTimeout(() => {
          autoAnalyze(instance!)
        }, 500) // 500ms debounce
      })

      // Initial analyze
      setTimeout(() => {
        if (instance) autoAnalyze(instance)
      }, 100)
    }

    init()

    return () => {
      if (analyzeTimeoutRef.current) {
        clearTimeout(analyzeTimeoutRef.current)
      }
      disposeChange?.dispose()
      instance?.dispose()
    }
  }, [autoAnalyze, wasmReady])

  // Auto-scroll to bottom when output changes
  useEffect(() => {
    if (outputEndRef.current) {
      outputEndRef.current.scrollIntoView({ behavior: 'smooth' })
    }
  }, [output])

  // Focus input when waiting for input
  useEffect(() => {
    if (vmState === 'waiting_for_input' && inputRef.current) {
      inputRef.current.focus()
    }
  }, [vmState])

  const onRun = () => {
    if (!editor || !wasmReady) {
      return
    }
    
    const code = editor.getValue()
    setOutput('')
    setVmState('running')
    setExecutionTime(null)
    
    try {
      const result = start_program(code) as VmStateInfo
      
      setOutput(result.output)
      setVmState(result.state as VmState)
      if (result.execution_time_ms !== undefined) {
        setExecutionTime(result.execution_time_ms)
      }
    } catch (e) {
      setOutput(`${e}`)
      setVmState('error')
      console.error('Run error:', e)
    }
  }

  const onStop = () => {
    try {
      stop_program()
      setVmState('idle')
      setOutput(prev => prev + '\n[Program stopped]')
    } catch (e) {
      console.error('Stop error:', e)
    }
  }

  const onSendInput = () => {
    if (!inputValue.trim() || vmState !== 'waiting_for_input') {
      return
    }

    try {
      const result = provide_input(inputValue) as VmStateInfo
      
      setOutput(prev => prev + result.output)
      setVmState(result.state as VmState)
      if (result.execution_time_ms !== undefined) {
        setExecutionTime(result.execution_time_ms)
      }
      setInputValue('')
    } catch (e) {
      setOutput(prev => prev + `\nError: ${e}`)
      setVmState('error')
      console.error('Input error:', e)
    }
  }

  const handleInputKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      onSendInput()
    }
  }

  return (
    <div className="min-h-screen bg-gradient-to-br from-black via-gray-900 to-black relative overflow-hidden">
      {/* Lighting effects */}
      <div className="absolute inset-0 bg-gradient-radial from-blue-900/20 via-transparent to-transparent opacity-50 pointer-events-none" />
      <div className="absolute top-0 left-1/4 w-96 h-96 bg-purple-500/10 rounded-full blur-3xl pointer-events-none" />
      <div className="absolute bottom-0 right-1/4 w-96 h-96 bg-blue-500/10 rounded-full blur-3xl pointer-events-none" />
      
      {/* Header */}
      <header className="relative z-10 border-b border-gray-800/50 bg-black/30 backdrop-blur-md px-6 py-4 flex items-center justify-between">
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <Code2 className="w-8 h-8 text-blue-400" />
            <h1 className="text-2xl font-bold bg-gradient-to-r from-blue-400 to-purple-400 bg-clip-text text-transparent">
              Pyhyeon
            </h1>
          </div>
          <div className="text-sm text-gray-400">
            Playground
          </div>
        </div>
        <nav className="flex items-center gap-4">
          <a 
            href="https://github.com/csh1668/pyhyeon" 
            target="_blank" 
            rel="noreferrer" 
            className="text-gray-400 hover:text-white transition-colors p-2 rounded-lg hover:bg-white/10"
            aria-label="GitHub"
          >
            <Github className="w-5 h-5" />
          </a>
        </nav>
      </header>

      {/* Main workspace */}
      <main className="relative z-10 h-[calc(100vh-80px)] flex">
        {/* Editor section */}
        <div 
          className="flex flex-col bg-black/20 backdrop-blur-md border-r border-gray-800/50"
          style={{ width: `${editorWidth}%` }}
        >
          <div className="flex items-center justify-between px-4 py-3 border-b border-gray-800/50 bg-black/10">
            <div className="flex items-center gap-3">
              <div className="flex items-center gap-2 text-sm text-gray-300">
                <Code2 className="w-4 h-4" />
                main.pyh
              </div>
              
              {/* Examples dropdown */}
              {examples.length > 0 && (
                <div className="flex items-center gap-2 border-l border-gray-700 pl-3">
                  <FileCode className="w-4 h-4 text-gray-400" />
                  <Select value={selectedExample} onValueChange={loadExample}>
                    <SelectTrigger className="w-[240px] h-8 bg-black/30 border-gray-700 text-gray-200 text-xs">
                      <SelectValue placeholder="예제 파일 선택..." />
                    </SelectTrigger>
                    <SelectContent className="bg-gray-900 border-gray-700 text-gray-200 max-h-[400px]">
                      {Object.entries(
                        examples.reduce((acc, ex) => {
                          if (!acc[ex.category]) acc[ex.category] = []
                          acc[ex.category].push(ex)
                          return acc
                        }, {} as Record<string, Example[]>)
                      ).map(([category, categoryExamples]) => (
                        <SelectGroup key={category}>
                          <SelectLabel className="text-gray-400">{category}</SelectLabel>
                          {categoryExamples.map(ex => (
                            <SelectItem 
                              key={ex.id} 
                              value={ex.id}
                              className="text-gray-200 focus:bg-gray-800 focus:text-white"
                            >
                              <span className="font-mono text-xs">{ex.name}</span>
                              <span className="text-gray-500 ml-2 text-xs">- {ex.description}</span>
                            </SelectItem>
                          ))}
                        </SelectGroup>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              )}
            </div>
            
            {vmState === 'running' || vmState === 'waiting_for_input' ? (
              <Button 
                onClick={onStop}
                className="bg-red-600 hover:bg-red-700 text-white"
                size="sm"
                title="Stop program"
              >
                <StopCircle className="w-4 h-4 mr-2" />
                Stop
              </Button>
            ) : (
              <Button 
                onClick={onRun}
                className="bg-green-600 hover:bg-green-700 text-white"
                size="sm"
                disabled={!wasmReady}
                title="Run code (or press Ctrl+Enter)"
              >
                {wasmReady ? (
                  <>
                    <Play className="w-4 h-4 mr-2" />
                    Run (Ctrl+Enter)
                  </>
                ) : (
                  <>
                    <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                    Loading...
                  </>
                )}
              </Button>
            )}
          </div>
          <div className="flex-1" ref={editorRef} />
        </div>

        {/* Resize handle */}
        <div 
          className={`w-1 bg-gray-700/50 hover:bg-blue-500/50 cursor-col-resize transition-colors ${
            isResizing ? 'bg-blue-500/70' : ''
          }`}
          onMouseDown={handleMouseDown}
        />

        {/* Right panel */}
        <div 
          className="flex flex-col bg-black/20 backdrop-blur-md"
          style={{ width: `${100 - editorWidth}%` }}
        >
          {/* Output section */}
          <div className="flex-1 flex flex-col">
            <div className="flex items-center gap-2 px-4 py-3 text-sm text-gray-300 border-b border-gray-800/30 bg-black/10">
              <Terminal className="w-4 h-4" />
              Output
              {vmState === 'waiting_for_input' && (
                <span className="text-xs text-yellow-500 ml-2">Waiting for input...</span>
              )}
              {vmState === 'running' && (
                <span className="text-xs text-blue-500 ml-2 flex items-center gap-1">
                  <Loader2 className="w-3 h-3 animate-spin" />
                  Running...
                </span>
              )}
            </div>
            <div className="flex-1 p-4 flex flex-col">
              <div className="flex-1 bg-black/20 p-4 rounded-lg border border-gray-800/50 overflow-auto backdrop-blur-sm">
                <pre className="text-sm text-gray-100 font-mono whitespace-pre-wrap break-words" dangerouslySetInnerHTML={{ __html: outputHtml || 'Run your code to see output here...' }} />
                {executionTime !== null && vmState === 'finished' && (
                  <div className="mt-3 pt-3 border-t border-gray-700/50">
                    <span className="text-xs text-gray-400">
                      Execution time: {executionTime.toFixed(3)}ms
                    </span>
                  </div>
                )}
                <div ref={outputEndRef} />
              </div>
              {vmState === 'waiting_for_input' && (
                <div className="mt-4 flex gap-2">
                  <input
                    ref={inputRef}
                    type="text"
                    value={inputValue}
                    onChange={(e) => setInputValue(e.target.value)}
                    onKeyDown={handleInputKeyDown}
                    placeholder="Enter input..."
                    className="flex-1 px-3 py-2 bg-black/20 backdrop-blur-sm border border-gray-700 rounded-lg text-sm text-gray-100 placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-green-500"
                  />
                  <Button
                    onClick={onSendInput}
                    size="sm"
                    className="bg-green-600 hover:bg-green-700"
                    disabled={!inputValue.trim()}
                  >
                    <Send className="w-4 h-4" />
                  </Button>
                </div>
              )}
            </div>
          </div>
        </div>
      </main>
    </div>
  )
}

export default App
