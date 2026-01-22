import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import ToolIndicator from '../../src/components/ToolIndicator.vue'

describe('ToolIndicator', () => {
  describe('Basic Rendering', () => {
    it('renders tool type correctly', () => {
      const wrapper = mount(ToolIndicator, {
        props: { type: 'Read' },
      })

      expect(wrapper.find('.tool-type').text()).toBe('Read')
    })

    it('renders path when provided', () => {
      const wrapper = mount(ToolIndicator, {
        props: {
          type: 'Read',
          path: '/path/to/file.txt',
        },
      })

      expect(wrapper.find('.tool-path').exists()).toBe(true)
      expect(wrapper.find('.tool-path').text()).toBe('/path/to/file.txt')
    })

    it('renders description when path is not provided', () => {
      const wrapper = mount(ToolIndicator, {
        props: {
          type: 'Task',
          description: 'Running background task',
        },
      })

      expect(wrapper.find('.tool-description').exists()).toBe(true)
      expect(wrapper.find('.tool-description').text()).toBe('Running background task')
    })

    it('renders summary when provided', () => {
      const wrapper = mount(ToolIndicator, {
        props: {
          type: 'Edit',
          path: '/path/to/file.txt',
          summary: 'Added 2 lines',
        },
      })

      expect(wrapper.find('.tool-summary').exists()).toBe(true)
      expect(wrapper.find('.tool-summary').text()).toBe('Added 2 lines')
    })
  })

  describe('Tool Status', () => {
    it('shows running status when isRunning is true', () => {
      const wrapper = mount(ToolIndicator, {
        props: {
          type: 'Bash',
          isRunning: true,
        },
      })

      const dot = wrapper.find('.tool-dot')
      expect(dot.classes()).toContain('running')
      // 橘色: #e67e22
      expect(dot.attributes('style')).toContain('#e67e22')
    })

    it('shows success status when output is provided', () => {
      const wrapper = mount(ToolIndicator, {
        props: {
          type: 'Bash',
          output: 'Command completed',
        },
      })

      const dot = wrapper.find('.tool-dot')
      expect(dot.classes()).not.toContain('running')
      // 綠色: #2ecc71
      expect(dot.attributes('style')).toContain('#2ecc71')
    })

    it('shows success status when exitCode is 0', () => {
      const wrapper = mount(ToolIndicator, {
        props: {
          type: 'Bash',
          exitCode: 0,
        },
      })

      const dot = wrapper.find('.tool-dot')
      // 綠色: #2ecc71
      expect(dot.attributes('style')).toContain('#2ecc71')
    })

    it('shows error status when exitCode is non-zero', () => {
      const wrapper = mount(ToolIndicator, {
        props: {
          type: 'Bash',
          exitCode: 1,
        },
      })

      const dot = wrapper.find('.tool-dot')
      // 紅色: #e74c3c
      expect(dot.attributes('style')).toContain('#e74c3c')
    })

    it('shows cancelled status when isCancelled is true', () => {
      const wrapper = mount(ToolIndicator, {
        props: {
          type: 'Edit',
          isCancelled: true,
        },
      })

      const dot = wrapper.find('.tool-dot')
      // 灰色: #a0a0a0
      expect(dot.attributes('style')).toContain('#a0a0a0')
    })

    it('cancelled status takes precedence over running', () => {
      const wrapper = mount(ToolIndicator, {
        props: {
          type: 'Bash',
          isRunning: true,
          isCancelled: true,
        },
      })

      const dot = wrapper.find('.tool-dot')
      // 應該是灰色（cancelled），不是橘色（running）
      expect(dot.attributes('style')).toContain('#a0a0a0')
    })
  })

  describe('Bash Tool Specific', () => {
    it('renders input command', () => {
      const wrapper = mount(ToolIndicator, {
        props: {
          type: 'Bash',
          input: 'npm install',
        },
      })

      expect(wrapper.find('.tool-block').exists()).toBe(true)
      expect(wrapper.find('.block-label').text()).toBe('IN')
      expect(wrapper.text()).toContain('npm install')
    })

    it('renders output with exit code', () => {
      const wrapper = mount(ToolIndicator, {
        props: {
          type: 'Bash',
          output: 'installed 50 packages',
          exitCode: 0,
        },
      })

      expect(wrapper.find('.tool-block.output').exists()).toBe(true)
      expect(wrapper.find('.exit-code').text()).toBe('Exit code 0')
      expect(wrapper.find('.exit-code').classes()).not.toContain('error')
    })

    it('shows error styling for non-zero exit code', () => {
      const wrapper = mount(ToolIndicator, {
        props: {
          type: 'Bash',
          output: 'command not found',
          exitCode: 127,
        },
      })

      expect(wrapper.find('.exit-code').text()).toBe('Exit code 127')
      expect(wrapper.find('.exit-code').classes()).toContain('error')
    })
  })

  describe('Edit Tool Specific', () => {
    it('renders side-by-side diff when oldCode and newCode provided', () => {
      const wrapper = mount(ToolIndicator, {
        props: {
          type: 'Edit',
          path: '/path/to/file.ts',
          oldCode: 'const x = 1;',
          newCode: 'const x = 2;',
        },
      })

      expect(wrapper.find('.diff-sidebyside').exists()).toBe(true)
      expect(wrapper.find('.diff-panel.old').exists()).toBe(true)
      expect(wrapper.find('.diff-panel.new').exists()).toBe(true)
    })
  })

  describe('User Response', () => {
    it('renders user response when provided', () => {
      const wrapper = mount(ToolIndicator, {
        props: {
          type: 'Edit',
          isCancelled: true,
          userResponse: 'Denied by user',
        },
      })

      expect(wrapper.find('.user-response').exists()).toBe(true)
      expect(wrapper.find('.response-label').text()).toBe('User:')
      expect(wrapper.text()).toContain('Denied by user')
    })
  })

  describe('Expand/Collapse', () => {
    it('is expanded by default', () => {
      const wrapper = mount(ToolIndicator, {
        props: {
          type: 'Bash',
          input: 'echo hello',
        },
      })

      expect(wrapper.find('.tool-content').exists()).toBe(true)
      expect(wrapper.find('.expand-icon').text()).toBe('▼')
    })

    it('collapses when header is clicked', async () => {
      const wrapper = mount(ToolIndicator, {
        props: {
          type: 'Bash',
          input: 'echo hello',
        },
      })

      await wrapper.find('.tool-header').trigger('click')

      expect(wrapper.find('.tool-content').exists()).toBe(false)
      expect(wrapper.find('.expand-icon').text()).toBe('▶')
    })

    it('expands again when header is clicked twice', async () => {
      const wrapper = mount(ToolIndicator, {
        props: {
          type: 'Bash',
          input: 'echo hello',
        },
      })

      // 第一次點擊：收合
      await wrapper.find('.tool-header').trigger('click')
      expect(wrapper.find('.tool-content').exists()).toBe(false)

      // 第二次點擊：展開
      await wrapper.find('.tool-header').trigger('click')
      expect(wrapper.find('.tool-content').exists()).toBe(true)
    })
  })

  describe('Reason', () => {
    it('renders reason when provided', () => {
      const wrapper = mount(ToolIndicator, {
        props: {
          type: 'Task',
          reason: 'Starting background agent',
        },
      })

      expect(wrapper.find('.tool-reason').exists()).toBe(true)
      expect(wrapper.find('.reason-label').text()).toBe('Reason:')
      expect(wrapper.text()).toContain('Starting background agent')
    })
  })
})
