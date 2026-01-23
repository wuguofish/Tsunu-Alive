import { describe, it, expect } from 'vitest'
import {
  generateFileReference,
  extractFileName,
  formatContextDisplay,
  getConnectionStatusText,
  type IdeContext,
} from '../../src/utils/ideUtils'

describe('ideUtils', () => {
  describe('generateFileReference', () => {
    it('returns null for null context', () => {
      expect(generateFileReference(null)).toBeNull()
    })

    it('returns null for context without file_path', () => {
      const ctx: IdeContext = {
        file_path: null,
        selected_text: null,
        selection: null,
        language_id: null,
        last_updated: null,
      }
      expect(generateFileReference(ctx)).toBeNull()
    })

    it('returns @filepath for context without selection', () => {
      const ctx: IdeContext = {
        file_path: '/src/App.vue',
        selected_text: null,
        selection: null,
        language_id: 'vue',
        last_updated: null,
      }
      expect(generateFileReference(ctx)).toBe('@/src/App.vue')
    })

    it('returns @filepath#L5 for single line selection', () => {
      const ctx: IdeContext = {
        file_path: 'D:\\game\\tsunu_alive\\src\\App.vue',
        selected_text: 'const x = 1;',
        selection: {
          start_line: 4, // 0-based, will be 5 in output
          start_character: 0,
          end_line: 4,
          end_character: 12,
        },
        language_id: 'vue',
        last_updated: null,
      }
      expect(generateFileReference(ctx)).toBe('@D:\\game\\tsunu_alive\\src\\App.vue#L5')
    })

    it('returns @filepath#L10-20 for multi-line selection', () => {
      const ctx: IdeContext = {
        file_path: '/src/utils/helper.ts',
        selected_text: 'function test() {\n  return true;\n}',
        selection: {
          start_line: 9, // 0-based, will be 10 in output
          start_character: 0,
          end_line: 19, // 0-based, will be 20 in output
          end_character: 1,
        },
        language_id: 'typescript',
        last_updated: null,
      }
      expect(generateFileReference(ctx)).toBe('@/src/utils/helper.ts#L10-20')
    })
  })

  describe('extractFileName', () => {
    it('returns empty string for null', () => {
      expect(extractFileName(null)).toBe('')
    })

    it('extracts filename from Unix path', () => {
      expect(extractFileName('/src/utils/helper.ts')).toBe('helper.ts')
    })

    it('extracts filename from Windows path', () => {
      expect(extractFileName('D:\\game\\tsunu_alive\\src\\App.vue')).toBe('App.vue')
    })

    it('handles filename without path', () => {
      expect(extractFileName('App.vue')).toBe('App.vue')
    })
  })

  describe('formatContextDisplay', () => {
    it('returns null for null context', () => {
      expect(formatContextDisplay(null)).toBeNull()
    })

    it('returns null for context without file_path', () => {
      const ctx: IdeContext = {
        file_path: null,
        selected_text: null,
        selection: null,
        language_id: null,
        last_updated: null,
      }
      expect(formatContextDisplay(ctx)).toBeNull()
    })

    it('returns filename for context without selection', () => {
      const ctx: IdeContext = {
        file_path: '/src/App.vue',
        selected_text: null,
        selection: null,
        language_id: 'vue',
        last_updated: null,
      }
      expect(formatContextDisplay(ctx)).toBe('App.vue')
    })

    it('returns filename:line for context with selection', () => {
      const ctx: IdeContext = {
        file_path: 'D:\\game\\src\\helper.ts',
        selected_text: 'code',
        selection: {
          start_line: 99, // 0-based, will be 100 in output
          start_character: 0,
          end_line: 105,
          end_character: 10,
        },
        language_id: 'typescript',
        last_updated: null,
      }
      expect(formatContextDisplay(ctx)).toBe('helper.ts:100')
    })
  })

  describe('getConnectionStatusText', () => {
    it('returns "IDE: Off" when not running', () => {
      expect(getConnectionStatusText(false, [])).toBe('IDE: Off')
    })

    it('returns "IDE: Waiting" when running but no clients', () => {
      expect(getConnectionStatusText(true, [])).toBe('IDE: Waiting')
    })

    it('returns "IDE: ClientName" for single client', () => {
      expect(getConnectionStatusText(true, [{ name: 'VS Code' }])).toBe('IDE: VS Code')
    })

    it('returns "IDE: N connected" for multiple clients', () => {
      expect(
        getConnectionStatusText(true, [{ name: 'VS Code' }, { name: 'PyCharm' }]),
      ).toBe('IDE: 2 connected')
    })

    it('returns correct count for three clients', () => {
      expect(
        getConnectionStatusText(true, [
          { name: 'VS Code' },
          { name: 'PyCharm' },
          { name: 'WebStorm' },
        ]),
      ).toBe('IDE: 3 connected')
    })
  })
})
