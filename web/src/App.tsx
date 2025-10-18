import { useEffect, useRef, useState, useCallback, useMemo } from 'react'
import * as monaco from 'monaco-editor'
import { Github, Play, Loader2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { initWasm } from './lib/utils'
import { analyze as wasmAnalyze, run as wasmRun } from '@pkg/pyhyeon'
import { AnsiUp } from 'ansi_up'

function App() {
  const editorRef = useRef<HTMLDivElement>(null)
  const [editor, setEditor] = useState<monaco.editor.IStandaloneCodeEditor | null>(null)
  const [output, setOutput] = useState<string>("")
  const analyzeTimeoutRef = useRef<number | null>(null)
  const [editorWidth, setEditorWidth] = useState<number>(60) // 60% default
  const [isResizing, setIsResizing] = useState(false)
  const [wasmReady, setWasmReady] = useState(false)
  
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
    const src = model.getValue()

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
          'editor.background': '#0a0a0a',
          'editor.foreground': '#fafafa',
          'editor.lineHighlightBackground': '#14141410',
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
          'if', 'elif', 'else', 'while', 'def', 'return', 
          'and', 'or', 'not'
        ],
        
        // 상수 키워드
        constants: ['None', 'True', 'False'],
        
        // 내장 함수들
        builtins: ['print', 'input', 'int', 'bool', 'str', 'len'],
        
        // 연산자들
        operators: [
          '=', '==', '!=', '<', '<=', '>', '>=',
          '+', '-', '*', '//', '%'
        ],
        
        // 구분자들
        delimiters: ['(', ')', ':', ',', ';'],
        
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
            
            // 키워드, 상수, 내장함수, 식별자
            [/[a-zA-Z_]\w*/, {
              cases: {
                '@keywords': 'keyword',
                '@constants': 'constant',
                '@builtins': 'builtin',
                '@default': 'identifier'
              }
            }],
            
            // 연산자
            [/==|!=|<=|>=|\/\/|[+\-*%<>=]/, 'operator'],
            
            // 구분자
            [/[()\:,;]/, 'delimiter'],
            
            // 공백
            [/[ \t\r\n]+/, 'white'],
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
        ],
        autoClosingPairs: [
          { open: '(', close: ')' },
          { open: '"', close: '"' },
          { open: "'", close: "'" },
        ],
        surroundingPairs: [
          { open: '(', close: ')' },
          { open: '"', close: '"' },
          { open: "'", close: "'" },
        ],
        comments: {
          lineComment: '#',
        },
        indentationRules: {
          increaseIndentPattern: /^.*:\s*$/,
          decreaseIndentPattern: /^(.*\s*)?$/,
        },
      })

      instance = monaco.editor.create(editorRef.current!, {
        value: 'def fib(n):\n  if n < 2:\n    return n\n  return fib(n-1) + fib(n-2)\n\nprint(fib(10))\n',
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
        try {
          const out = wasmRun(code)
          setOutput(out || 'Program executed successfully with no output.')
        } catch (e) {
          setOutput(`${e}`)
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

  const onRun = () => {
    if (!editor || !wasmReady) {
      return
    }
    
    const code = editor.getValue()
    try {
      const out = wasmRun(code)
      setOutput(out || 'Program executed successfully with no output.')
    } catch (e) {
      setOutput(`${e}`)
      console.error('Run error:', e)
    }
  }

  return (
    <div className="app-root">
      <header className="app-header">
        <div className="flex items-center gap-4">
          <div className="brand text-xl font-bold">Pyhyeon Playground</div>
        </div>
        <nav className="flex items-center gap-4">
          <a 
            href="https://github.com/csh1668/pyhyeon" 
            target="_blank" 
            rel="noreferrer" 
            className="text-muted-foreground hover:text-foreground transition-colors"
            aria-label="GitHub"
          >
            <Github className="w-5 h-5" />
          </a>
        </nav>
      </header>
      
      <main className="workspace" style={{ gridTemplateColumns: `${editorWidth}% 4px ${100 - editorWidth}%` }}>
        <div className="editor-section">
          <div className="editor-container">
            <div className="editor-pane" ref={editorRef} />
            <Button 
              onClick={onRun}
              className="floating-run-button bg-green-600 hover:bg-green-700"
              size="sm"
              title="Run code (or press Ctrl+Enter)"
              disabled={!wasmReady}
            >
              {wasmReady ? (
                <>
                  <Play className="w-4 h-4 mr-1" />
                  Run
                </>
              ) : (
                <>
                  <Loader2 className="w-4 h-4 mr-1 animate-spin" />
                  Loading...
                </>
              )}
            </Button>
          </div>
        </div>
        
        <div 
          className={`resize-handle ${isResizing ? 'resizing' : ''}`}
          onMouseDown={handleMouseDown}
        />
        
        <div className="output-section">
          <div className="section-header">
            <span className="text-sm font-medium">Output</span>
          </div>
          <div className="output-container">
            <pre className="output-content" dangerouslySetInnerHTML={{ __html: outputHtml }} />
          </div>
        </div>
      </main>
    </div>
  )
}

export default App
