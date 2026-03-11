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
    editMode: 'default',
    contextUsage: null,
    contextInfo: null,
    lastPrompt: '',
    availableSkills: [],
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
      state.editMode = 'default'

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
        originalPrompt: '',  // state.lastPrompt 預設為空字串
      })
      expect(result.stateUpdates.avatarState).toBe('waiting')
      expect(result.stateUpdates.busyStatus).toBe('等待確認...')
    })

    it('adds tool to deniedToolsThisRequest', () => {
      const state = createDefaultState()
      state.editMode = 'default'

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
      state.editMode = 'default'
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

    it('sets pendingPermission when editMode is acceptEdits', () => {
      // acceptEdits 模式下仍然會顯示權限對話框（針對 Bash 等工具）
      const state = createDefaultState()
      state.editMode = 'acceptEdits'

      const event: ClaudeEvent = {
        event_type: 'PermissionDenied',
        tool_name: 'Edit',
        tool_id: 'tool-123',
        input: { file_path: '/test.txt' },
      }

      const result = handlePermissionDeniedEvent(event, state)

      expect(result.stateUpdates.pendingPermission).toEqual({
        toolName: 'Edit',
        toolId: 'tool-123',
        input: { file_path: '/test.txt' },
        originalPrompt: '',  // state.lastPrompt 預設為空字串
      })
    })

    it('auto-skips ExitPlanMode in plan mode (meta tool, uses Hook for confirmation)', () => {
      // ExitPlanMode 是 META_TOOL，PermissionDenied 會被自動跳過
      // 用戶確認計畫是通過 Hook 機制（plan-approval-request 事件）觸發的
      const state = createDefaultState()
      state.editMode = 'plan'

      const event: ClaudeEvent = {
        event_type: 'PermissionDenied',
        tool_name: 'ExitPlanMode',
        tool_id: 'tool-exit-plan',
        input: { plan: 'My implementation plan...' },
      }

      const result = handlePermissionDeniedEvent(event, state)

      // ExitPlanMode 是 META_TOOL，會自動跳過
      expect(result.stateUpdates.pendingPermission).toBeUndefined()
      expect(result.stateUpdates.deniedToolsThisRequest?.has('ExitPlanMode')).toBe(true)
    })

    it('triggers stopBusyTextAnimation when setting pendingPermission', () => {
      const state = createDefaultState()
      state.editMode = 'default'

      const event: ClaudeEvent = {
        event_type: 'PermissionDenied',
        tool_name: 'Edit',
        tool_id: 'tool-123',
      }

      const result = handlePermissionDeniedEvent(event, state)

      expect(result.actions).toContainEqual({ type: 'stopBusyTextAnimation' })
    })

    it('clears streamingText to prevent text duplication after permission dialog', () => {
      const state = createDefaultState()
      state.editMode = 'default'
      state.streamingText = '之前累積的文字'

      const event: ClaudeEvent = {
        event_type: 'PermissionDenied',
        tool_name: 'Bash',
        tool_id: 'tool-456',
      }

      const result = handlePermissionDeniedEvent(event, state)

      expect(result.stateUpdates.streamingText).toBe('')
    })

    // AUTO_ALLOW_TOOLS 相關測試
    describe('auto-allow tools (legacy fallback mode)', () => {
      it('auto-allows AskUserQuestion without showing permission dialog', () => {
        const state = createDefaultState()

        const event: ClaudeEvent = {
          event_type: 'PermissionDenied',
          tool_name: 'AskUserQuestion',
          tool_id: 'tool-ask-123',
        }

        const result = handlePermissionDeniedEvent(event, state)

        // 不應該設定 pendingPermission（不顯示對話框）
        expect(result.stateUpdates.pendingPermission).toBeUndefined()
        // 應該加入 deniedToolsThisRequest（用於重新執行時允許）
        expect(result.stateUpdates.deniedToolsThisRequest?.has('AskUserQuestion')).toBe(true)
      })

      it('auto-allows Read tool without showing permission dialog', () => {
        const state = createDefaultState()

        const event: ClaudeEvent = {
          event_type: 'PermissionDenied',
          tool_name: 'Read',
          tool_id: 'tool-read-123',
          input: { file_path: '/test.txt' },
        }

        const result = handlePermissionDeniedEvent(event, state)

        // Read 是 AUTO_ALLOW_TOOL，自動跳過不顯示對話框
        expect(result.stateUpdates.pendingPermission).toBeUndefined()
        expect(result.stateUpdates.deniedToolsThisRequest?.has('Read')).toBe(true)
      })

      it('auto-allows Glob tool without showing permission dialog', () => {
        const state = createDefaultState()

        const event: ClaudeEvent = {
          event_type: 'PermissionDenied',
          tool_name: 'Glob',
          tool_id: 'tool-glob-123',
          input: { pattern: '**/*.ts' },
        }

        const result = handlePermissionDeniedEvent(event, state)

        // Glob 是 AUTO_ALLOW_TOOL，自動跳過不顯示對話框
        expect(result.stateUpdates.pendingPermission).toBeUndefined()
        expect(result.stateUpdates.deniedToolsThisRequest?.has('Glob')).toBe(true)
      })

      it('auto-allows Grep tool without showing permission dialog', () => {
        const state = createDefaultState()

        const event: ClaudeEvent = {
          event_type: 'PermissionDenied',
          tool_name: 'Grep',
          tool_id: 'tool-grep-123',
          input: { pattern: 'test' },
        }

        const result = handlePermissionDeniedEvent(event, state)

        // Grep 是 AUTO_ALLOW_TOOL，自動跳過不顯示對話框
        expect(result.stateUpdates.pendingPermission).toBeUndefined()
        expect(result.stateUpdates.deniedToolsThisRequest?.has('Grep')).toBe(true)
      })

      it('auto-allows Task tool without showing permission dialog', () => {
        const state = createDefaultState()

        const event: ClaudeEvent = {
          event_type: 'PermissionDenied',
          tool_name: 'Task',
          tool_id: 'tool-task-123',
        }

        const result = handlePermissionDeniedEvent(event, state)

        expect(result.stateUpdates.pendingPermission).toBeUndefined()
        expect(result.stateUpdates.deniedToolsThisRequest?.has('Task')).toBe(true)
      })

      it('auto-allows TodoRead and TodoWrite without showing permission dialog', () => {
        const state = createDefaultState()

        const readEvent: ClaudeEvent = {
          event_type: 'PermissionDenied',
          tool_name: 'TodoRead',
          tool_id: 'tool-todo-read',
        }

        const writeEvent: ClaudeEvent = {
          event_type: 'PermissionDenied',
          tool_name: 'TodoWrite',
          tool_id: 'tool-todo-write',
        }

        const readResult = handlePermissionDeniedEvent(readEvent, state)
        expect(readResult.stateUpdates.pendingPermission).toBeUndefined()
        expect(readResult.stateUpdates.deniedToolsThisRequest?.has('TodoRead')).toBe(true)

        // 重置 state
        const state2 = createDefaultState()
        const writeResult = handlePermissionDeniedEvent(writeEvent, state2)
        expect(writeResult.stateUpdates.pendingPermission).toBeUndefined()
        expect(writeResult.stateUpdates.deniedToolsThisRequest?.has('TodoWrite')).toBe(true)
      })

      it('auto-allows WebSearch and WebFetch without showing permission dialog', () => {
        const state = createDefaultState()

        const searchEvent: ClaudeEvent = {
          event_type: 'PermissionDenied',
          tool_name: 'WebSearch',
          tool_id: 'tool-websearch',
          input: { query: 'test' },
        }

        const result = handlePermissionDeniedEvent(searchEvent, state)
        // WebSearch 是 AUTO_ALLOW_TOOL，自動跳過不顯示對話框
        expect(result.stateUpdates.pendingPermission).toBeUndefined()
        expect(result.stateUpdates.deniedToolsThisRequest?.has('WebSearch')).toBe(true)

        const state2 = createDefaultState()
        const fetchEvent: ClaudeEvent = {
          event_type: 'PermissionDenied',
          tool_name: 'WebFetch',
          tool_id: 'tool-webfetch',
          input: { url: 'https://example.com' },
        }

        const result2 = handlePermissionDeniedEvent(fetchEvent, state2)
        // WebFetch 是 AUTO_ALLOW_TOOL，自動跳過不顯示對話框
        expect(result2.stateUpdates.pendingPermission).toBeUndefined()
        expect(result2.stateUpdates.deniedToolsThisRequest?.has('WebFetch')).toBe(true)
      })

      it('auto-allows EnterPlanMode without showing permission dialog', () => {
        const state = createDefaultState()

        const event: ClaudeEvent = {
          event_type: 'PermissionDenied',
          tool_name: 'EnterPlanMode',
          tool_id: 'tool-enter-plan',
        }

        const result = handlePermissionDeniedEvent(event, state)

        expect(result.stateUpdates.pendingPermission).toBeUndefined()
        expect(result.stateUpdates.deniedToolsThisRequest?.has('EnterPlanMode')).toBe(true)
      })

      it('still shows permission dialog for Edit tool (not auto-allowed)', () => {
        const state = createDefaultState()

        const event: ClaudeEvent = {
          event_type: 'PermissionDenied',
          tool_name: 'Edit',
          tool_id: 'tool-edit-123',
          input: { file_path: '/test.txt' },
        }

        const result = handlePermissionDeniedEvent(event, state)

        // Edit 不在 AUTO_ALLOW 列表中，應該顯示對話框
        expect(result.stateUpdates.pendingPermission).toBeDefined()
        expect(result.stateUpdates.pendingPermission?.toolName).toBe('Edit')
      })

      it('still shows permission dialog for Bash tool (not auto-allowed)', () => {
        const state = createDefaultState()

        const event: ClaudeEvent = {
          event_type: 'PermissionDenied',
          tool_name: 'Bash',
          tool_id: 'tool-bash-123',
        }

        const result = handlePermissionDeniedEvent(event, state)

        expect(result.stateUpdates.pendingPermission).toBeDefined()
        expect(result.stateUpdates.pendingPermission?.toolName).toBe('Bash')
      })

      it('auto-allows ExitPlanMode without showing permission dialog (meta tool)', () => {
        const state = createDefaultState()

        const event: ClaudeEvent = {
          event_type: 'PermissionDenied',
          tool_name: 'ExitPlanMode',
          tool_id: 'tool-exit-plan',
          input: { plan: 'Implementation plan...' },
        }

        const result = handlePermissionDeniedEvent(event, state)

        // ExitPlanMode 是 META_TOOL，會自動跳過
        // 注意：ExitPlanMode 的確認對話框是由 Hook 機制（plan-approval-request）觸發的，不是 PermissionDenied
        expect(result.stateUpdates.pendingPermission).toBeUndefined()
        expect(result.stateUpdates.deniedToolsThisRequest?.has('ExitPlanMode')).toBe(true)
      })
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

    it('preserves user answer when userAnswered is true (AskUserQuestion)', () => {
      // 模擬用戶已回答 AskUserQuestion 的情境
      const userAnswer = 'Question 1: User answer here'
      const state = createDefaultState()
      state.currentToolUses = [
        {
          id: 'tool-ask-123',
          type: 'AskUserQuestion',
          name: 'AskUserQuestion',
          input: { questions: [] },
          result: userAnswer,
          userAnswered: true,  // 用戶已回答
        },
      ]
      state.messages = [
        {
          role: 'assistant',
          items: [
            {
              type: 'tool',
              tool: {
                id: 'tool-ask-123',
                type: 'AskUserQuestion',
                name: 'AskUserQuestion',
                input: { questions: [] },
                result: userAnswer,
                userAnswered: true,
              },
            },
          ],
        },
      ]

      // Claude CLI 返回的 ToolResult（應該被忽略）
      const event: ClaudeEvent = {
        event_type: 'ToolResult',
        tool_id: 'tool-ask-123',
        result: 'Answer questions?',  // 這是 Claude CLI 的預設回應，應該被忽略
      }

      const result = handleToolResultEvent(event, state)

      // 用戶的答案應該保留，不被覆蓋
      expect(result.stateUpdates.currentToolUses![0].result).toBe(userAnswer)

      // messages 中的工具也應該保留用戶答案
      const toolItem = result.stateUpdates.messages![0].items[0]
      if (toolItem.type === 'tool') {
        expect(toolItem.tool.result).toBe(userAnswer)
      }
    })

    it('updates result normally when userAnswered is false', () => {
      const state = createDefaultState()
      state.currentToolUses = [
        {
          id: 'tool-123',
          type: 'Read',
          name: 'Read',
          input: {},
          userAnswered: false,  // 沒有用戶回答
        },
      ]
      state.messages = [
        {
          role: 'assistant',
          items: [
            {
              type: 'tool',
              tool: {
                id: 'tool-123',
                type: 'Read',
                name: 'Read',
                input: {},
                userAnswered: false,
              },
            },
          ],
        },
      ]

      const event: ClaudeEvent = {
        event_type: 'ToolResult',
        tool_id: 'tool-123',
        result: 'File content',
      }

      const result = handleToolResultEvent(event, state)

      // 結果應該被正常更新
      expect(result.stateUpdates.currentToolUses![0].result).toBe('File content')
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

    it('updates context usage when provided', () => {
      const state = createDefaultState()
      const event: ClaudeEvent = {
        event_type: 'Complete',
        context_window_used_percent: 45.7,
      }

      const result = handleCompleteEvent(event, state)

      expect(result.stateUpdates.contextUsage).toBe(46) // 四捨五入
    })

    it('updates context info when provided', () => {
      const state = createDefaultState()
      const event: ClaudeEvent = {
        event_type: 'Complete',
        total_tokens_in_conversation: 15000,
        context_window_max: 200000,
      }

      const result = handleCompleteEvent(event, state)

      expect(result.stateUpdates.contextInfo).toEqual({
        totalTokens: 15000,
        maxTokens: 200000,
      })
    })

    it('does not set context fields when not provided', () => {
      const state = createDefaultState()
      const event: ClaudeEvent = {
        event_type: 'Complete',
        cost_usd: 0.01,
      }

      const result = handleCompleteEvent(event, state)

      expect(result.stateUpdates.contextUsage).toBeUndefined()
      expect(result.stateUpdates.contextInfo).toBeUndefined()
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
      expect(result.stateUpdates.avatarState).toBe('error')
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
