import { useState, useEffect } from 'react'
import * as monaco from 'monaco-editor'

export function useIsMobile(editor: monaco.editor.IStandaloneCodeEditor | null) {
  const [isMobile, setIsMobile] = useState(window.innerWidth < 768)
  
  useEffect(() => {
    const handleResize = () => {
      const mobile = window.innerWidth < 768
      setIsMobile(mobile)
      
      if (editor) {
        editor.updateOptions({
          fontSize: mobile ? 12 : 14,
          lineHeight: mobile ? 20 : 24,
          padding: { top: mobile ? 30 : 50, bottom: mobile ? 12 : 16 }
        })
      }
    }
    
    window.addEventListener('resize', handleResize)
    return () => window.removeEventListener('resize', handleResize)
  }, [editor])
  
  return isMobile
}

