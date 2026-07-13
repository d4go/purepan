<template>
  <el-dialog
    v-model="visible"
    title="分享文件"
    width="480px"
    :close-on-click-modal="false"
    @close="handleClose"
  >
    <!-- 待分享文件列表 -->
    <div class="file-list-section-pc">
      <div class="section-label">待分享文件 ({{ files.length }})</div>
      <div class="file-list-pc">
        <div v-for="file in files" :key="file.path" class="file-item-pc">
          <el-icon class="file-icon">
            <Folder v-if="file.isdir === 1" />
            <Document v-else />
          </el-icon>
          <span class="file-name">{{ file.server_filename }}</span>
        </div>
      </div>
    </div>

    <!-- 分享配置表单 - PC端 -->
    <el-form v-if="!shareResult" label-width="80px" class="share-form-pc">
      <el-form-item label="有效期">
        <el-select v-model="form.period" style="width: 100%">
          <el-option label="永久" :value="0" />
          <el-option label="1天" :value="1" />
          <el-option label="7天" :value="7" />
          <el-option label="30天" :value="30" />
        </el-select>
      </el-form-item>
      <el-form-item label="提取码">
        <div class="pwd-input-row">
          <el-input
            v-model="form.pwd"
            maxlength="4"
            placeholder="4位提取码"
            style="width: 200px"
            :class="{ 'is-error': pwdError }"
          />
          <el-button @click="handleGeneratePwd" style="margin-left: 10px">随机生成</el-button>
        </div>
        <div v-if="pwdError" class="error-tip">{{ pwdError }}</div>
      </el-form-item>
    </el-form>

    <!-- 分享结果展示 - PC端 -->
    <div v-if="shareResult" class="share-result-pc">
      <div class="result-row">
        <span class="label">链接:</span>
        <span class="value">{{ shareResult.link }}</span>
      </div>
      <div class="result-row">
        <span class="label">提取码:</span>
        <span class="value pwd-highlight">{{ shareResult.pwd }}</span>
      </div>
      <div class="result-actions">
        <el-button @click="copyLink" :icon="Link">复制链接</el-button>
        <el-button type="primary" @click="copyAll" :icon="DocumentCopy">复制链接和提取码</el-button>
      </div>
    </div>

    <!-- 错误提示 -->
    <el-alert
      v-if="errorMessage"
      :title="errorMessage"
      type="error"
      show-icon
      :closable="false"
      class="error-alert"
    />

    <template #footer>
      <div class="dialog-footer">
        <el-button @click="handleClose">{{ shareResult ? '关闭' : '取消' }}</el-button>
        <el-button
          v-if="!shareResult"
          type="primary"
          @click="handleShare"
          :loading="loading"
        >
          创建分享
        </el-button>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { Link, DocumentCopy, Refresh, Folder, Document } from '@element-plus/icons-vue'
import { createShare, generatePwd, type ShareResult } from '@/api/share'
import type { FileItem } from '@/api/file'

// Props 定义
const props = defineProps<{
  /** 对话框显示控制 */
  modelValue: boolean
  /** 待分享文件列表 */
  files: FileItem[]
}>()

// Emits 定义
const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  'success': [result: ShareResult]
}>()

// 对话框可见性
const visible = computed({
  get: () => props.modelValue,
  set: (val) => emit('update:modelValue', val),
})

// 表单数据
const form = ref({
  period: 7,  // 默认7天有效期
  pwd: '',
})

// 状态
const loading = ref(false)
const errorMessage = ref('')
const pwdError = ref('')
const shareResult = ref<ShareResult | null>(null)

// 生成随机提取码
function handleGeneratePwd() {
  form.value.pwd = generatePwd()
  pwdError.value = ''
}

// 验证提取码格式
function validatePwd(): boolean {
  const pwd = form.value.pwd.trim()
  if (pwd && pwd.length !== 4) {
    pwdError.value = '提取码必须是4位字符'
    return false
  }
  // 验证只包含字母和数字
  if (pwd && !/^[a-zA-Z0-9]{4}$/.test(pwd)) {
    pwdError.value = '提取码只能包含字母和数字'
    return false
  }
  pwdError.value = ''
  return true
}

// 创建分享
async function handleShare() {
  // 验证提取码
  if (!validatePwd()) {
    return
  }

  // 验证文件列表
  if (props.files.length === 0) {
    errorMessage.value = '请选择要分享的文件'
    return
  }

  loading.value = true
  errorMessage.value = ''

  try {
    // 获取文件路径列表
    const paths = props.files.map(file => file.path)
    
    // 如果没有提取码，自动生成
    let pwd = form.value.pwd.trim()
    if (!pwd) {
      pwd = generatePwd()
      form.value.pwd = pwd
    }

    const result = await createShare({
      paths,
      period: form.value.period,
      pwd,
    })

    shareResult.value = result
    emit('success', result)
    ElMessage.success('分享创建成功')
  } catch (error: any) {
    errorMessage.value = error.message || '创建分享失败，请稍后重试'
  } finally {
    loading.value = false
  }
}

// 复制链接
async function copyLink() {
  if (!shareResult.value) return
  
  try {
    await navigator.clipboard.writeText(shareResult.value.link)
    ElMessage.success('链接已复制到剪贴板')
  } catch {
    ElMessage.error('复制失败，请手动复制')
  }
}

// 复制链接和提取码
async function copyAll() {
  if (!shareResult.value) return

  const { link, pwd } = shareResult.value;

  const text = pwd
      ? `链接: ${link}?pwd=${pwd}\n提取码: ${pwd}`
      : `链接: ${link}`;
  try {
    await navigator.clipboard.writeText(text)
    ElMessage.success('链接和提取码已复制到剪贴板')
  } catch {
    ElMessage.error('复制失败，请手动复制')
  }
}

// 关闭对话框
function handleClose() {
  visible.value = false
}

// 重置状态
function resetState() {
  form.value = {
    period: 7,
    pwd: '',
  }
  loading.value = false
  errorMessage.value = ''
  pwdError.value = ''
  shareResult.value = null
}

// 监听对话框关闭，重置状态
watch(visible, (newVal) => {
  if (!newVal) {
    // 延迟重置，避免关闭动画时看到内容变化
    setTimeout(resetState, 300)
  }
})

// 监听提取码输入，清除错误
watch(() => form.value.pwd, () => {
  if (pwdError.value) {
    pwdError.value = ''
  }
})
</script>

<style scoped lang="scss">
/* =====================
   通用样式
   ===================== */
.section-label {
  font-size: 14px;
  color: var(--el-text-color-secondary);
  margin-bottom: 8px;
}

.file-icon {
  color: var(--el-color-primary);
  margin-right: 8px;
  flex-shrink: 0;
}

.file-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.error-tip {
  font-size: 12px;
  color: var(--el-color-danger);
  margin-top: 4px;
}

.error-alert {
  margin-top: 16px;
}

.is-error {
  :deep(.el-input__wrapper) {
    box-shadow: 0 0 0 1px var(--el-color-danger) inset;
  }
}

/* =====================
   PC端样式
   ===================== */
.file-list-section-pc {
  margin-bottom: 20px;
}

.file-list-pc {
  max-height: 150px;
  overflow-y: auto;
  background: var(--el-fill-color-light);
  border-radius: 8px;
  padding: 8px 12px;
}

.file-item-pc {
  display: flex;
  align-items: center;
  padding: 4px 0;
  font-size: 13px;
  
  &:not(:last-child) {
    border-bottom: 1px solid var(--el-border-color-lighter);
  }
}

.share-form-pc {
  margin-top: 16px;
}

.pwd-input-row {
  display: flex;
  align-items: center;
}

.share-result-pc {
  margin-top: 20px;
  padding: 16px;
  background: var(--el-fill-color-light);
  border-radius: 8px;
}

.share-result-pc .result-row {
  margin-bottom: 12px;
  display: flex;
  align-items: flex-start;
  
  &:last-of-type {
    margin-bottom: 16px;
  }
}

.share-result-pc .label {
  color: var(--el-text-color-secondary);
  width: 60px;
  flex-shrink: 0;
}

.share-result-pc .value {
  word-break: break-all;
}

.share-result-pc .pwd-highlight {
  color: var(--el-color-success);
  font-weight: bold;
  font-size: 16px;
}

.share-result-pc .result-actions {
  display: flex;
  gap: 12px;
  margin-top: 16px;
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
}

:deep(.el-dialog) {
  border-radius: 14px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.12);

  .el-dialog__header {
    border-bottom: 1px solid var(--app-divider);
  }

  .el-dialog__footer {
    border-top: 1px solid var(--app-divider);
  }
}
</style>
