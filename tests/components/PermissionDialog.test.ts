import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import PermissionDialog from '../../src/components/PermissionDialog.vue'

describe('PermissionDialog', () => {
  const defaultProps = {
    action: 'Edit',
    target: '/path/to/file.txt',
  }

  it('renders action and target correctly', () => {
    const wrapper = mount(PermissionDialog, {
      props: defaultProps,
    })

    expect(wrapper.find('.action-badge').text()).toBe('Edit')
    expect(wrapper.find('.target-path').text()).toBe('/path/to/file.txt')
    expect(wrapper.find('.question').text()).toContain('欸，要執行 Edit 嗎？')
  })

  it('renders summary when provided', () => {
    const wrapper = mount(PermissionDialog, {
      props: {
        ...defaultProps,
        summary: 'Added 2 lines',
      },
    })

    expect(wrapper.find('.summary').exists()).toBe(true)
    expect(wrapper.find('.summary').text()).toBe('Added 2 lines')
  })

  it('does not render summary when not provided', () => {
    const wrapper = mount(PermissionDialog, {
      props: defaultProps,
    })

    expect(wrapper.find('.summary').exists()).toBe(false)
  })

  it('renders preview when provided', () => {
    const wrapper = mount(PermissionDialog, {
      props: {
        ...defaultProps,
        preview: 'const x = 1;',
      },
    })

    expect(wrapper.find('.preview-section').exists()).toBe(true)
    expect(wrapper.find('.preview-code code').text()).toBe('const x = 1;')
  })

  it('does not render preview when not provided', () => {
    const wrapper = mount(PermissionDialog, {
      props: defaultProps,
    })

    expect(wrapper.find('.preview-section').exists()).toBe(false)
  })

  it('emits "yes" response when clicking first button', async () => {
    const wrapper = mount(PermissionDialog, {
      props: defaultProps,
    })

    await wrapper.find('.option-btn.primary').trigger('click')

    expect(wrapper.emitted('respond')).toBeTruthy()
    expect(wrapper.emitted('respond')![0]).toEqual(['yes'])
  })

  it('emits "yes-all" response when clicking second button', async () => {
    const wrapper = mount(PermissionDialog, {
      props: defaultProps,
    })

    const buttons = wrapper.findAll('.option-btn')
    await buttons[1].trigger('click')

    expect(wrapper.emitted('respond')).toBeTruthy()
    expect(wrapper.emitted('respond')![0]).toEqual(['yes-all'])
  })

  it('emits "yes-always" response when clicking third button', async () => {
    const wrapper = mount(PermissionDialog, {
      props: defaultProps,
    })

    const buttons = wrapper.findAll('.option-btn')
    await buttons[2].trigger('click')

    expect(wrapper.emitted('respond')).toBeTruthy()
    expect(wrapper.emitted('respond')![0]).toEqual(['yes-always'])
  })

  it('emits "no" response when clicking fourth button', async () => {
    const wrapper = mount(PermissionDialog, {
      props: defaultProps,
    })

    const buttons = wrapper.findAll('.option-btn')
    await buttons[3].trigger('click')

    expect(wrapper.emitted('respond')).toBeTruthy()
    expect(wrapper.emitted('respond')![0]).toEqual(['no'])
  })

  it('shows custom input when clicking custom toggle button', async () => {
    const wrapper = mount(PermissionDialog, {
      props: defaultProps,
    })

    // 初始狀態沒有自訂輸入框
    expect(wrapper.find('.custom-input-wrapper').exists()).toBe(false)
    expect(wrapper.find('.custom-toggle').exists()).toBe(true)

    // 點擊自訂按鈕
    await wrapper.find('.custom-toggle').trigger('click')

    // 自訂輸入框應該顯示
    expect(wrapper.find('.custom-input-wrapper').exists()).toBe(true)
    expect(wrapper.find('.custom-toggle').exists()).toBe(false)
  })

  it('emits "custom" response with message when submitting custom input', async () => {
    const wrapper = mount(PermissionDialog, {
      props: defaultProps,
    })

    // 顯示自訂輸入框
    await wrapper.find('.custom-toggle').trigger('click')

    // 輸入自訂訊息
    const input = wrapper.find('.custom-input')
    await input.setValue('不要刪除這個檔案，改成修改它')

    // 點擊送出按鈕
    await wrapper.find('.submit-custom').trigger('click')

    expect(wrapper.emitted('respond')).toBeTruthy()
    expect(wrapper.emitted('respond')![0]).toEqual(['custom', '不要刪除這個檔案，改成修改它'])
  })

  it('does not emit custom response when input is empty', async () => {
    const wrapper = mount(PermissionDialog, {
      props: defaultProps,
    })

    // 顯示自訂輸入框
    await wrapper.find('.custom-toggle').trigger('click')

    // 不輸入任何內容，直接點擊送出
    await wrapper.find('.submit-custom').trigger('click')

    // 不應該 emit 任何事件
    expect(wrapper.emitted('respond')).toBeFalsy()
  })

  it('trims whitespace from custom message', async () => {
    const wrapper = mount(PermissionDialog, {
      props: defaultProps,
    })

    // 顯示自訂輸入框
    await wrapper.find('.custom-toggle').trigger('click')

    // 輸入帶有空白的訊息
    const input = wrapper.find('.custom-input')
    await input.setValue('  請改用其他方法  ')

    // 點擊送出按鈕
    await wrapper.find('.submit-custom').trigger('click')

    expect(wrapper.emitted('respond')).toBeTruthy()
    expect(wrapper.emitted('respond')![0]).toEqual(['custom', '請改用其他方法'])
  })

  it('disables submit button when custom input is empty or whitespace', async () => {
    const wrapper = mount(PermissionDialog, {
      props: defaultProps,
    })

    // 顯示自訂輸入框
    await wrapper.find('.custom-toggle').trigger('click')

    const submitBtn = wrapper.find('.submit-custom')

    // 空輸入，按鈕應該禁用
    expect((submitBtn.element as HTMLButtonElement).disabled).toBe(true)

    // 只有空白，按鈕應該禁用
    await wrapper.find('.custom-input').setValue('   ')
    expect((submitBtn.element as HTMLButtonElement).disabled).toBe(true)

    // 有內容，按鈕應該啟用
    await wrapper.find('.custom-input').setValue('有內容')
    expect((submitBtn.element as HTMLButtonElement).disabled).toBe(false)
  })
})
