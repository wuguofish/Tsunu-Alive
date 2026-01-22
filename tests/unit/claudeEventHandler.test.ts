import { describe, it, expect } from 'vitest'
import {
  handleInitEvent,
  handleTextEvent,
  handleToolUseEvent,
  handlePermissionDeniedEvent,
  handleToolResultEvent,
  handleCompleteEvent,
  handleErrorEvent,
  handleConnectedEvent,
  handleClaudeEvent,
  type AppState,
  type ClaudeEvent,
} from '../../src/utils/claudeEventHandler'

// 建立預設的測試狀態
function createDefaultState(): AppState {
  return {
    sessionId: null,
    currentModel: '',
    streamingText: '',
    messages: [],
    currentToolUses: [],
    deniedToolsThisRequest: new Set(),
    pendingPermission: null,
    avatarState: 'idle',
    busyStatus: '',
    isLoading: false,
    editMode: 'ask',
  }
}

describe('claudeEventHandler', () => {
  describe('handleInitEvent', () => {
    it('updates sessionId and model', () => {
      const state = createDefaultState()
      const event: ClaudeEvent = {
        event_type: 'Init',
        session_id: 'test-session-123',
        model: 'claude-sonnet-4',
      }

      const result = handleInitEvent(event, state)

      expect(result.stateUpdates.sessionId).toBe('test-session-123')
      expect(result.stateUpdates.currentModel).toBe('claude-sonnet-4')
      expect(result.stateUpdates.busyStatus).toBe('Thinking...')
    })

    it('only updates provided fields', () => {
      const state = createDefaultState()
      const event: ClaudeEvent = {
        event_type: 'Init',
        session_id: 'test-session',
      }

      const result = handleInitEvent(event, state)

      expect(result.stateUpdates.sessionId).toBe('test-session')
      expect(result.stateUpdates.currentModel).toBeUndefined()
    })
  })

  describe('handleTextEvent', () => {
    it('accumulates streaming text', () => {
      const state = createDefaultState()
      state.streamingText = 'Hello '

      const event: ClaudeEvent = {
        event_type: 'Text',
        text: 'World!',
      }

      const result = handleTextEvent(event, state)

      expect(result.stateUpdates.streamingText).toBe('Hello World!')
    })

    it('creates new assistant message if none exists', () => {
      const state = createDefaultState()
      const event: ClaudeEvent = {
        event_type: 'Text',
        text: 'Hi there!',
      }

      const result = handleTextEvent(event, state)

      expect(result.stateUpdates.messages).toHaveLength(1)
      expect(result.stateUpdates.messages![0].role).toBe('assistant')
      expect(result.stateUpdates.messages![0].items[0]).toEqual({
        type: 'text',
        content: 'Hi there!',
      })
    })

    it('appends to existing assistant message', () => {
      const state = createDefaultState()
      state.messages = [
        { role: 'assistant', items: [{ type: 'text', content: 'First ' }] },
      ]
      state.streamingText = 'First '

      const event: ClaudeEvent = {
        event_type: 'Text',
        text: 'Second',
      }

      const result = handleTextEvent(event, state)

      expect(result.stateUpdates.messages).toHaveLength(1)
      expect(result.stateUpdates.messages![0].items[0]).toEqual({
        type: 'text',
        content: 'First Second',
      })
    })

    it('adds text after tool use', () => {
      const state = createDefaultState()
      state.messages = [
        {
          role: 'assistant',
          items: [
            {
              type: 'tool',
              tool: { id: 't1', type: 'Read', name: 'Read', input: {} },
            },
          ],
        },
      ]

      const event: ClaudeEvent = {
        event_type: 'Text',
        text: 'After tool',
      }

      const result = handleTextEvent(event, state)

      expect(result.stateUpdates.messages![0].items).toHaveLength(2)
      expect(result.stateUpdates.messages![0].items[1]).toEqual({
        type: 'text',
        content: 'After tool',
      })
    })

    it('triggers scrollToBottom action', () => {
      const state = createDefaultState()
      const event: ClaudeEvent = {
        event_type: 'Text',
        text: 'Hello',
      }

      const result = handleTextEvent(event, state)

      expect(result.actions).toContainEqual({ type: 'scrollToBottom' })
    })

    it('does nothing when text is empty', () => {
      const state = createDefaultState()
      const event: ClaudeEvent = {
        event_type: 'Text',
      }

      const result = handleTextEvent(event, state)

      expect(result.stateUpdates).toEqual({})
      expect(result.actions).toEqual([])
    })
  })

  describe('handleToolUseEvent', () => {
    it('adds new tool to messages', () => {
      const state = createDefaultState()
      state.messages = [{ role: 'assistant', items: [] }]

      const event: ClaudeEvent = {
        event_type: 'ToolUse',
        tool_name: 'Read',
        tool_id: 'tool-123',
        input: { file_path: '/test.txt' },
      }

      const result = handleToolUseEvent(event, state)

      expect(result.stateUpdates.messages![0].items).toHaveLength(1)
      const toolItem = result.stateUpdates.messages![0].items[0]
      expect(toolItem.type).toBe('tool')
      if (toolItem.type === 'tool') {
        expect(toolItem.tool.id).toBe('tool-123')
        expect(toolItem.tool.name).toBe('Read')
        expect(toolItem.tool.input).toEqual({ file_path: '/test.txt' })
      }
    })

    it('updates busyStatus', () => {
      const state = createDefaultState()
      const event: ClaudeEvent = {
        event_type: 'ToolUse',
        tool_name: 'Bash',
        tool_id: 'tool-456',
      }

      const result = handleToolUseEvent(event, state)

      expect(result.stateUpdates.busyStatus).toBe('Using Bash...')
    })

    it('does not add duplicate tools', () => {
      const state = createDefaultState()
      state.messages = [
        {
          role: 'assistant',
          items: [
            {
              type: 'tool',
              tool: { id: 'tool-123', type: 'Read', name: 'Read', input: {} },
            },
          ],
        },
      ]

      const event: ClaudeEvent = {
        event_type: 'ToolUse',
        tool_name: 'Read',
        tool_id: 'tool-123',
      }

      const result = handleToolUseEvent(event, state)

      // items 數量應該不變
      expect(result.stateUpdates.messages![0].items).toHaveLength(1)
    })

    it('does nothing when tool_name or tool_id is missing', () => {
      const state = createDefaultState()
      const event: ClaudeEvent = {
        event_type: 'ToolUse',
        tool_name: 'Read',
        // missing tool_id
      }

      const result = handleToolUseEvent(event, state)

      expect(result.stateUpdates).toEqual({})
    })
  })

  describe('handlePermissionDeniedEvent', () => {
    it('sets pendingPermission when editMode is ask', () => {
      const state = createDefaultState()
      state.editMode = 'ask'

      const event: ClaudeEvent = {
        event_type: 'PermissionDenied',
        tool_name: 'Edit',
        tool_id: 'tool-789',
        input: { file_path: '/edit.txt' },
      }

      const result = handlePermissionDeniedEvent(event, state)

      expect(result.stateUpdates.pendingPermission).toEqual({
        toolName: 'Edit',
        toolId: 'tool-789',
        input: { file_path: '/edit.txt' },
      })
      expect(result.stateUpdates.avatarState).toBe('waiting')
      expect(result.stateUpdates.busyStatus).toBe('等待確認...')
    })

    it('adds tool to deniedToolsThisRequest', () => {
      const state = createDefaultState()
      state.editMode = 'ask'

      const event: ClaudeEvent = {
        event_type: 'PermissionDenied',
        tool_name: 'Bash',
        tool_id: 'tool-111',
      }

      const result = handlePermissionDeniedEvent(event, state)

      expect(result.stateUpdates.deniedToolsThisRequest?.has('Bash')).toBe(true)
    })

    it('does not overwrite existing pendingPermission', () => {
      const state = createDefaultState()
      state.editMode = 'ask'
      state.pendingPermission = {
        toolName: 'Edit',
        toolId: 'existing-tool',
        input: {},
      }

      const event: ClaudeEvent = {
        event_type: 'PermissionDenied',
        tool_name: 'Bash',
        tool_id: 'new-tool',
      }

      const result = handlePermissionDeniedEvent(event, state)

      // pendingPermission 應該保持不變
      expect(result.stateUpdates.pendingPermission).toBeUndefined()
    })

    it('does nothing when editMode is not ask', () => {
      const state = createDefaultState()
      state.editMode = 'auto'

      const event: ClaudeEvent = {
        event_type: 'PermissionDenied',
        tool_name: 'Edit',
        tool_id: 'tool-123',
      }

      const result = handlePermissionDeniedEvent(event, state)

      expect(result.stateUpdates).toEqual({})
    })

    it('triggers stopBusyTextAnimation when setting pendingPermission', () => {
      const state = createDefaultState()
      state.editMode = 'ask'

      const event: ClaudeEvent = {
        event_type: 'PermissionDenied',
        tool_name: 'Edit',
        tool_id: 'tool-123',
      }

      const result = handlePermissionDeniedEvent(event, state)

      expect(result.actions).toContainEqual({ type: 'stopBusyTextAnimation' })
    })
  })

  describe('handleToolResultEvent', () => {
    it('updates tool result in currentToolUses', () => {
      const state = createDefaultState()
      state.currentToolUses = [
        { id: 'tool-123', type: 'Read', name: 'Read', input: {} },
      ]

      const event: ClaudeEvent = {
        event_type: 'ToolResult',
        tool_id: 'tool-123',
        result: 'File content here',
      }

      const result = handleToolResultEvent(event, state)

      expect(result.stateUpdates.currentToolUses![0].result).toBe('File content here')
    })

    it('marks tool as cancelled on error', () => {
      const state = createDefaultState()
      state.currentToolUses = [
        { id: 'tool-123', type: 'Bash', name: 'Bash', input: {} },
      ]
      state.messages = [
        {
          role: 'assistant',
          items: [
            {
              type: 'tool',
              tool: { id: 'tool-123', type: 'Bash', name: 'Bash', input: {} },
            },
          ],
        },
      ]

      const event: ClaudeEvent = {
        event_type: 'ToolResult',
        tool_id: 'tool-123',
        result: 'Command failed',
        is_error: true,
      }

      const result = handleToolResultEvent(event, state)

      expect(result.stateUpdates.currentToolUses![0].isCancelled).toBe(true)
    })

    it('does nothing when tool_id is missing', () => {
      const state = createDefaultState()
      const event: ClaudeEvent = {
        event_type: 'ToolResult',
        result: 'Some result',
      }

      const result = handleToolResultEvent(event, state)

      expect(result.stateUpdates).toEqual({})
    })
  })

  describe('handleCompleteEvent', () => {
    it('resets loading state and clears streaming text', () => {
      const state = createDefaultState()
      state.isLoading = true
      state.streamingText = 'Some text'

      const event: ClaudeEvent = {
        event_type: 'Complete',
        cost_usd: 0.05,
      }

      const result = handleCompleteEvent(event, state)

      expect(result.stateUpdates.isLoading).toBe(false)
      expect(result.stateUpdates.avatarState).toBe('complete')
      expect(result.stateUpdates.streamingText).toBe('')
    })

    it('triggers necessary actions', () => {
      const state = createDefaultState()
      const event: ClaudeEvent = { event_type: 'Complete' }

      const result = handleCompleteEvent(event, state)

      expect(result.actions).toContainEqual({ type: 'stopBusyTextAnimation' })
      expect(result.actions).toContainEqual({ type: 'startCompleteTimer' })
    })
  })

  describe('handleErrorEvent', () => {
    it('resets state and adds error action', () => {
      const state = createDefaultState()
      state.isLoading = true

      const event: ClaudeEvent = {
        event_type: 'Error',
        message: 'Connection failed',
      }

      const result = handleErrorEvent(event, state)

      expect(result.stateUpdates.isLoading).toBe(false)
      expect(result.stateUpdates.avatarState).toBe('idle')
      expect(result.actions).toContainEqual({
        type: 'addErrorMessage',
        message: 'Connection failed',
      })
    })
  })

  describe('handleConnectedEvent', () => {
    it('updates busyStatus', () => {
      const state = createDefaultState()
      const event: ClaudeEvent = { event_type: 'Connected' }

      const result = handleConnectedEvent(event, state)

      expect(result.stateUpdates.busyStatus).toBe('Connected')
    })
  })

  describe('handleClaudeEvent (main dispatcher)', () => {
    it('dispatches to correct handler based on event_type', () => {
      const state = createDefaultState()

      const initEvent: ClaudeEvent = {
        event_type: 'Init',
        session_id: 'test',
      }
      expect(handleClaudeEvent(initEvent, state).stateUpdates.sessionId).toBe('test')

      const textEvent: ClaudeEvent = {
        event_type: 'Text',
        text: 'Hello',
      }
      expect(handleClaudeEvent(textEvent, state).stateUpdates.streamingText).toBe('Hello')

      const completeEvent: ClaudeEvent = { event_type: 'Complete' }
      expect(handleClaudeEvent(completeEvent, state).stateUpdates.isLoading).toBe(false)
    })

    it('returns empty result for unknown event type', () => {
      const state = createDefaultState()
      const event = { event_type: 'Unknown' } as unknown as ClaudeEvent

      const result = handleClaudeEvent(event, state)

      expect(result.stateUpdates).toEqual({})
      expect(result.actions).toEqual([])
    })
  })
})
