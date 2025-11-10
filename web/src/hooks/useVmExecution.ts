import { useReducer, useCallback } from 'react'
import { start_program, provide_input, stop_program } from '@pkg/pyhyeon'

type VmState = 'idle' | 'running' | 'waiting_for_input' | 'finished' | 'error'

interface VmStateInfo {
  state: string
  output: string
  execution_time_ms?: number
}

interface VmExecutionState {
  vmState: VmState
  output: string
  executionTime: number | null
  inputValue: string
}

type VmAction = 
  | { type: 'START_PROGRAM' }
  | { type: 'PROGRAM_RESULT'; payload: VmStateInfo }
  | { type: 'STOP_PROGRAM' }
  | { type: 'SEND_INPUT'; payload: string }
  | { type: 'INPUT_RESULT'; payload: VmStateInfo }
  | { type: 'SET_INPUT_VALUE'; payload: string }
  | { type: 'ERROR'; payload: string }
  | { type: 'RESET' }

const initialState: VmExecutionState = {
  vmState: 'idle',
  output: '',
  executionTime: null,
  inputValue: ''
}

function vmExecutionReducer(state: VmExecutionState, action: VmAction): VmExecutionState {
  switch (action.type) {
    case 'START_PROGRAM':
      return {
        ...state,
        vmState: 'running',
        output: '',
        executionTime: null
      }
    
    case 'PROGRAM_RESULT':
      // Rust에서 이미 에러 메시지에 빨간색 ANSI 코드를 포함하여 전송하므로 그대로 사용
      return {
        ...state,
        output: action.payload.output,
        vmState: action.payload.state as VmState,
        executionTime: action.payload.execution_time_ms ?? null
      }
    
    case 'STOP_PROGRAM':
      return {
        ...state,
        vmState: 'idle',
        output: state.output + '\n[Program stopped]'
      }
    
    case 'SEND_INPUT':
      return {
        ...state,
        inputValue: ''
      }
    
    case 'INPUT_RESULT':
      // Rust에서 이미 에러 처리를 하므로 그대로 추가
      return {
        ...state,
        output: state.output + action.payload.output,
        vmState: action.payload.state as VmState,
        executionTime: action.payload.execution_time_ms ?? state.executionTime,
        inputValue: ''
      }
    
    case 'SET_INPUT_VALUE':
      return {
        ...state,
        inputValue: action.payload
      }
    
    case 'ERROR':
      return {
        ...state,
        vmState: 'error',
        output: state.output + (state.output ? '\n' : '') + action.payload
      }
    
    case 'RESET':
      return initialState
    
    default:
      return state
  }
}

export function useVmExecution() {
  const [state, dispatch] = useReducer(vmExecutionReducer, initialState)

  const startProgram = useCallback((code: string) => {
    dispatch({ type: 'START_PROGRAM' })
    
    try {
      const result = start_program(code) as VmStateInfo
      dispatch({ type: 'PROGRAM_RESULT', payload: result })
    } catch (e) {
      // ANSI red color code
      const errorMessage = `\x1b[31mError: ${e}\x1b[0m`
      dispatch({ type: 'ERROR', payload: errorMessage })
      console.error('Run error:', e)
    }
  }, [])

  const stopProgram = useCallback(() => {
    try {
      stop_program()
      dispatch({ type: 'STOP_PROGRAM' })
    } catch (e) {
      console.error('Stop error:', e)
    }
  }, [])

  const sendInput = useCallback((input: string) => {
    if (!input.trim() || state.vmState !== 'waiting_for_input') {
      return
    }

    try {
      const result = provide_input(input) as VmStateInfo
      dispatch({ type: 'INPUT_RESULT', payload: result })
    } catch (e) {
      // ANSI red color code
      const errorMessage = `\x1b[31mError: ${e}\x1b[0m`
      dispatch({ type: 'ERROR', payload: errorMessage })
      console.error('Input error:', e)
    }
  }, [state.vmState])

  const setInputValue = useCallback((value: string) => {
    dispatch({ type: 'SET_INPUT_VALUE', payload: value })
  }, [])

  const resetVm = useCallback(() => {
    dispatch({ type: 'RESET' })
  }, [])

  return {
    vmState: state.vmState,
    output: state.output,
    executionTime: state.executionTime,
    inputValue: state.inputValue,
    startProgram,
    stopProgram,
    sendInput,
    setInputValue,
    resetVm
  }
}

