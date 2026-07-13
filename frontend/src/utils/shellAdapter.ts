import { ElMessage } from 'element-plus'

export function getParentFolder(filePath: string): string {
  const separatorIndex = Math.max(filePath.lastIndexOf('/'), filePath.lastIndexOf('\\'))
  if (separatorIndex < 0) return ''

  const parentWithSeparator = filePath.slice(0, separatorIndex + 1)
  if (/^[A-Za-z]:[\\/]$/.test(parentWithSeparator)) {
    return parentWithSeparator
  }

  if (separatorIndex === 0) return filePath[0]
  return filePath.slice(0, separatorIndex)
}

export async function openFolder(path: string): Promise<void> {
  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) {
    try {
      const { open } = await import('@tauri-apps/plugin-shell')
      await open(path)
    } catch (err) {
      console.error('打开文件夹失败:', err)
      ElMessage.error('打开文件夹失败，请手动打开')
    }
  } else {
    // Web / Docker 环境：复制路径到剪贴板
    try {
      await navigator.clipboard.writeText(path)
      ElMessage.success(`路径已复制到剪贴板: ${path}`)
    } catch {
      ElMessage.info(`文件位置: ${path}`)
    }
  }
}
