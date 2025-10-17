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
          { token: 'comment', foreground: '6a9955', fontStyle: 'italic' },
          { token: 'keyword', foreground: 'c586c0' },
          { token: 'number', foreground: 'b5cea8' },
          { token: 'string', foreground: 'ce9178' },
          { token: 'operator', foreground: 'd4d4d4' },
          { token: 'delimiter', foreground: 'cccccc' },
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

      // 언어 등록(간단 Monarch)
      monaco.languages.register({ id: 'pyh' })
      monaco.languages.setMonarchTokensProvider('pyh', {
        keywords: ['if','elif','else','while','def','return','and','or','not','None','True','False'],
        operators: ['+','-','*','//','%','==','!=','<','<=','>','>=','='],
        tokenizer: {
          root: [
            [/#[^\n]*/, 'comment'],
            [/\d+/, 'number'],
            [/==|!=|<=|>=|\/\/|[%+\-*<>=]/, 'operator'],
            [/\b(if|elif|else|while|def|return|and|or|not|None|True|False)\b/, 'keyword'],
            [/\(|\)|\:|\,|\;/, 'delimiter'],
            [/\p{XID_Start}\p{XID_Continue}*/u, 'identifier'],
          ],
        },
      } as any)

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
        hover: {
          enabled: true,
          delay: 300,
          sticky: true,
          above: false, // Prefer showing hover below the line
        },
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
