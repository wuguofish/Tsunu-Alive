import { describe, it, expect } from 'vitest'
import {
  AUTO_ALLOW_TOOLS,
  isAutoAllowTool,
  AUTO_ALLOW_CATEGORIES,
} from '../../src/constants/autoAllowTools'

describe('autoAllowTools', () => {
  describe('AUTO_ALLOW_TOOLS', () => {
    it('contains user interaction tools', () => {
      expect(AUTO_ALLOW_TOOLS).toContain('AskUserQuestion')
    })

    it('contains task management tools', () => {
      expect(AUTO_ALLOW_TOOLS).toContain('TodoRead')
      expect(AUTO_ALLOW_TOOLS).toContain('TodoWrite')
    })

    it('contains read-only tools', () => {
      expect(AUTO_ALLOW_TOOLS).toContain('Read')
      expect(AUTO_ALLOW_TOOLS).toContain('Glob')
      expect(AUTO_ALLOW_TOOLS).toContain('Grep')
    })

    it('contains subagent tools', () => {
      expect(AUTO_ALLOW_TOOLS).toContain('Task')
      expect(AUTO_ALLOW_TOOLS).toContain('TaskOutput')
    })

    it('contains web read-only tools', () => {
      expect(AUTO_ALLOW_TOOLS).toContain('WebSearch')
      expect(AUTO_ALLOW_TOOLS).toContain('WebFetch')
    })

    it('contains plan mode tools', () => {
      expect(AUTO_ALLOW_TOOLS).toContain('EnterPlanMode')
    })

    it('does NOT contain write/edit tools', () => {
      expect(AUTO_ALLOW_TOOLS).not.toContain('Edit')
      expect(AUTO_ALLOW_TOOLS).not.toContain('Write')
      expect(AUTO_ALLOW_TOOLS).not.toContain('Bash')
      expect(AUTO_ALLOW_TOOLS).not.toContain('NotebookEdit')
    })

    it('does NOT contain ExitPlanMode (requires user confirmation)', () => {
      // ExitPlanMode 需要用戶確認計畫
      expect(AUTO_ALLOW_TOOLS).not.toContain('ExitPlanMode')
    })

    it('has no duplicates', () => {
      const uniqueTools = new Set(AUTO_ALLOW_TOOLS)
      expect(uniqueTools.size).toBe(AUTO_ALLOW_TOOLS.length)
    })
  })

  describe('isAutoAllowTool', () => {
    it('returns true for tools in the list', () => {
      expect(isAutoAllowTool('AskUserQuestion')).toBe(true)
      expect(isAutoAllowTool('Read')).toBe(true)
      expect(isAutoAllowTool('Glob')).toBe(true)
      expect(isAutoAllowTool('Grep')).toBe(true)
      expect(isAutoAllowTool('Task')).toBe(true)
      expect(isAutoAllowTool('TodoRead')).toBe(true)
      expect(isAutoAllowTool('WebSearch')).toBe(true)
      expect(isAutoAllowTool('EnterPlanMode')).toBe(true)
    })

    it('returns false for tools NOT in the list', () => {
      expect(isAutoAllowTool('Edit')).toBe(false)
      expect(isAutoAllowTool('Write')).toBe(false)
      expect(isAutoAllowTool('Bash')).toBe(false)
      expect(isAutoAllowTool('ExitPlanMode')).toBe(false)
      expect(isAutoAllowTool('NotebookEdit')).toBe(false)
      expect(isAutoAllowTool('KillShell')).toBe(false)
    })

    it('returns false for empty string', () => {
      expect(isAutoAllowTool('')).toBe(false)
    })

    it('returns false for non-existent tools', () => {
      expect(isAutoAllowTool('NonExistentTool')).toBe(false)
      expect(isAutoAllowTool('FakeTool123')).toBe(false)
    })

    it('is case sensitive', () => {
      expect(isAutoAllowTool('read')).toBe(false) // 'Read' is correct
      expect(isAutoAllowTool('READ')).toBe(false)
      expect(isAutoAllowTool('askuserquestion')).toBe(false)
    })
  })

  describe('AUTO_ALLOW_CATEGORIES', () => {
    it('has all expected categories', () => {
      expect(AUTO_ALLOW_CATEGORIES).toHaveProperty('userInteraction')
      expect(AUTO_ALLOW_CATEGORIES).toHaveProperty('taskManagement')
      expect(AUTO_ALLOW_CATEGORIES).toHaveProperty('readOnly')
      expect(AUTO_ALLOW_CATEGORIES).toHaveProperty('subagent')
      expect(AUTO_ALLOW_CATEGORIES).toHaveProperty('webReadOnly')
      expect(AUTO_ALLOW_CATEGORIES).toHaveProperty('planMode')
    })

    it('categories combined equal AUTO_ALLOW_TOOLS', () => {
      const allFromCategories = [
        ...AUTO_ALLOW_CATEGORIES.userInteraction,
        ...AUTO_ALLOW_CATEGORIES.taskManagement,
        ...AUTO_ALLOW_CATEGORIES.readOnly,
        ...AUTO_ALLOW_CATEGORIES.subagent,
        ...AUTO_ALLOW_CATEGORIES.webReadOnly,
        ...AUTO_ALLOW_CATEGORIES.planMode,
      ]

      // 數量應該相同
      expect(allFromCategories.length).toBe(AUTO_ALLOW_TOOLS.length)

      // 內容應該相同
      for (const tool of allFromCategories) {
        expect(AUTO_ALLOW_TOOLS).toContain(tool)
      }
    })
  })

  describe('Sync with Rust backend', () => {
    // 這個測試用來提醒開發者需要同步 Rust 後端
    // 實際的同步驗證需要在 CI/CD 中進行

    it('documents expected tools count for sync verification', () => {
      // 如果這個數字改變，需要同步更新 permission_server.rs
      // 目前：AskUserQuestion, TodoRead, TodoWrite, Read, Glob, Grep,
      //       Task, TaskOutput, WebSearch, WebFetch, EnterPlanMode
      expect(AUTO_ALLOW_TOOLS.length).toBe(11)
    })

    it('lists all tools for easy comparison with Rust', () => {
      // 方便開發者與 permission_server.rs 對照
      const expectedTools = [
        'AskUserQuestion',
        'TodoRead',
        'TodoWrite',
        'Read',
        'Glob',
        'Grep',
        'Task',
        'TaskOutput',
        'WebSearch',
        'WebFetch',
        'EnterPlanMode',
      ]

      expect([...AUTO_ALLOW_TOOLS].sort()).toEqual(expectedTools.sort())
    })
  })
})
