<template>
  <div class="file-list">
    <!-- PC端表格视图 -->
    <table class="file-table">
      <thead>
      <tr>
        <th v-if="multiple" class="col-checkbox"></th>
        <th class="col-name">名称</th>
        <th class="col-time">修改日期</th>
        <th class="col-type">类型</th>
        <th class="col-size">大小</th>
      </tr>
      </thead>
      <tbody>
      <FileItem
          v-for="entry in entries"
          :key="entry.id"
          :entry="entry"
          :selected="multiple ? isMultiSelected(entry) : selection?.id === entry.id"
          :disabled="isDisabled(entry)"
          :show-checkbox="multiple"
          @click="handleClick"
          @dblclick="handleDblClick"
          @checkbox-change="handleCheckboxChange"
      />
      </tbody>
    </table>
  </div>
</template>

<script setup lang="ts">
import type { FileEntry } from '@/api/filesystem'
import FileItem from './FileItem.vue'

const props = defineProps<{
  entries: FileEntry[]
  selection: FileEntry | null
  multiSelection?: FileEntry[]
  selectType?: 'file' | 'directory' | 'both'
  multiple?: boolean
}>()

const emit = defineEmits<{
  'select': [entry: FileEntry]
  'open': [entry: FileEntry]
  'toggle-select': [entry: FileEntry]
  'select-all': []
  'clear-selection': []
}>()

// 检查条目是否在多选列表中
function isMultiSelected(entry: FileEntry): boolean {
  return props.multiSelection?.some(e => e.id === entry.id) ?? false
}

// 检查条目是否禁用
function isDisabled(entry: FileEntry): boolean {
  if (props.selectType === 'file' && entry.entryType === 'directory') {
    return false // 文件夹不禁用，允许双击进入
  }
  if (props.selectType === 'directory' && entry.entryType === 'file') {
    return true
  }
  return false
}

// 单击选择
function handleClick(entry: FileEntry) {
  if (props.multiple) {
    emit('toggle-select', entry)
  } else {
    emit('select', entry)
  }
}

// 双击打开
function handleDblClick(entry: FileEntry) {
  emit('open', entry)
}

// 复选框变化
function handleCheckboxChange(entry: FileEntry) {
  emit('toggle-select', entry)
}

// 获取文件类型文本
function getTypeText(entry: FileEntry): string {
  if (entry.entryType === 'directory') {
    if (/^[A-Za-z]:$/.test(entry.name)) {
      return '本地磁盘'
    }
    return '文件夹'
  }

  const ext = entry.name.split('.').pop()?.toLowerCase()
  if (!ext) return '文件'

  const typeMap: Record<string, string> = {
    jpg: 'JPEG 图像', jpeg: 'JPEG 图像', png: 'PNG 图像', gif: 'GIF 图像',
    mp4: 'MP4 视频', avi: 'AVI 视频', mkv: 'MKV 视频',
    mp3: 'MP3 音频', wav: 'WAV 音频',
    pdf: 'PDF 文档', doc: 'Word 文档', docx: 'Word 文档',
    xls: 'Excel 表格', xlsx: 'Excel 表格',
    txt: '文本文件',
    zip: 'ZIP 压缩', rar: 'RAR 压缩',
    exe: '可执行文件',
  }

  return typeMap[ext] || `${ext.toUpperCase()} 文件`
}

// 格式化文件大小
function formatSize(bytes: number | null): string {
  if (bytes === null || bytes === undefined) return '-'
  if (bytes === 0) return '0 B'

  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))

  return (bytes / Math.pow(k, i)).toFixed(i > 0 ? 1 : 0) + ' ' + sizes[i]
}

</script>

<style scoped>
.file-list {
  height: 100%;
  overflow-y: auto;
}

.multi-select-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  background: var(--app-surface-hover);
  border-bottom: 1px solid var(--el-border-color);
}

.multi-select-toolbar .el-button {
  font-size: 13px;
}

.file-table {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
}

.file-table thead {
  position: sticky;
  top: 0;
  background: var(--el-fill-color-light);
  z-index: 1;
}

.file-table th {
  padding: 8px 12px;
  text-align: left;
  font-weight: 500;
  font-size: 13px;
  color: var(--el-text-color-secondary);
  border-bottom: 1px solid var(--el-border-color);
  user-select: none;
}

.col-checkbox {
  width: 40px;
  text-align: center;
}

.col-name {
  min-width: 200px;
  max-width: 40%;
}

.col-time {
  width: 23%;
}

.col-type {
  width: 15%;
}

.col-size {
  width: 12%;
  text-align: right !important;
}

.file-table :deep(td:last-child) {
  text-align: right;
}

</style>