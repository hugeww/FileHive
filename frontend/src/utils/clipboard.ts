/**
 * 剪贴板工具函数
 */

import { buildRetrieveUrl, buildWgetCommand } from '@/utils/share-url'

type CopyNotifyType = 'success' | 'error'

interface CopyOptions {
  successMsg?: string
  errorMsg?: string
  showMsg?: boolean
  notify?: (message: string, type: CopyNotifyType) => void
}

/**
 * 复制文本到剪贴板
 * @param text 要复制的文本
 * @param options 配置选项
 * @returns Promise<boolean> 是否复制成功
 */
export const copyToClipboard = async (
  text: string,
  options: CopyOptions = {}
): Promise<boolean> => {
  const {
    successMsg = '复制成功',
    errorMsg = '复制失败，请手动复制',
    showMsg = true,
    notify
  } = options

  const showCopyMessage = (message: string, type: CopyNotifyType) => {
    if (showMsg) {
      notify?.(message, type)
    }
  }

  // 1. 优先尝试现代 Clipboard API（仅在受保护环境如 HTTPS/localhost 下且可用时）
  if (navigator?.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text)
      showCopyMessage(successMsg, 'success')
      return true
    } catch {
      // 若浏览器（如 Safari/无焦点窗口）拒绝权限，平滑降级到传统 textarea 方案
    }
  }

  // 2. 传统兼容方案（针对 HTTP、iOS Safari、移动端 WebView 等环境）
  try {
    const textarea = document.createElement('textarea')
    textarea.value = text
    // 避免 iOS 屏幕弹跳与滚动
    textarea.style.position = 'fixed'
    textarea.style.top = '0'
    textarea.style.left = '0'
    textarea.style.width = '2em'
    textarea.style.height = '2em'
    textarea.style.padding = '0'
    textarea.style.border = 'none'
    textarea.style.outline = 'none'
    textarea.style.boxShadow = 'none'
    textarea.style.background = 'transparent'
    textarea.setAttribute('readonly', '')

    document.body.appendChild(textarea)
    textarea.focus()
    textarea.select()
    textarea.setSelectionRange(0, textarea.value.length)

    const success = document.execCommand('copy')
    document.body.removeChild(textarea)

    if (success) {
      showCopyMessage(successMsg, 'success')
      return true
    }
  } catch (err) {
    console.warn('execCommand copy fallback error:', err)
  }

  showCopyMessage(errorMsg, 'error')
  return false
}

/**
 * 生成并复制取件链接
 * @param code 取件码
 * @returns Promise<boolean> 是否复制成功
 */
export const copyRetrieveLink = async (
  code: string,
  options: Pick<CopyOptions, 'notify' | 'showMsg'> = {}
): Promise<boolean> => {
  const link = buildRetrieveUrl(code)
  return copyToClipboard(link, {
    successMsg: '取件链接已复制到剪贴板',
    errorMsg: '复制失败，请手动复制取件链接',
    ...options
  })
}

/**
 * 复制取件码
 * @param code 取件码
 * @returns Promise<boolean> 是否复制成功
 */
export const copyRetrieveCode = async (
  code: string,
  options: Pick<CopyOptions, 'notify' | 'showMsg'> = {}
): Promise<boolean> => {
  return copyToClipboard(code, {
    successMsg: '取件码已复制到剪贴板',
    errorMsg: '复制失败，请手动复制取件码',
    ...options
  })
}

export const copyWgetCommand = (
  retrieveCode: string,
  fileName: string,
  options: Pick<CopyOptions, 'notify' | 'showMsg'> = {}
) => {
  const command = buildWgetCommand(retrieveCode, fileName)
  void copyToClipboard(command, {
    successMsg: '命令已复制到剪贴板',
    errorMsg: '复制失败，请手动复制命令',
    ...options
  })
}
