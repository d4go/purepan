<template>
  <div class="conflict-strategy-selector">
    <el-form-item :label="label">
      <el-select
        :model-value="modelValue"
        @update:model-value="$emit('update:modelValue', $event)"
        :placeholder="placeholder"
        style="width: 100%"
      >
        <el-option
          v-for="option in options"
          :key="option.value"
          :label="option.label"
          :value="option.value"
        >
          <div class="strategy-option">
            <span class="strategy-label">{{ option.label }}</span>
            <el-tooltip :content="option.description" placement="right" :show-after="300">
              <el-icon class="info-icon"><InfoFilled /></el-icon>
            </el-tooltip>
          </div>
        </el-option>
      </el-select>
      <div v-if="showDescription && selectedOption" class="strategy-description">
        <el-icon><InfoFilled /></el-icon>
        {{ selectedOption.description }}
      </div>
    </el-form-item>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { InfoFilled } from '@element-plus/icons-vue'
import type { UploadConflictStrategy } from '@/api/upload'
import type { DownloadConflictStrategy } from '@/api/download'

interface StrategyOption {
  value: string
  label: string
  description: string
}

interface Props {
  modelValue?: UploadConflictStrategy | DownloadConflictStrategy
  type: 'upload' | 'download'
  label?: string
  placeholder?: string
  showDescription?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  label: '冲突策略',
  placeholder: '请选择冲突处理策略',
  showDescription: true
})

defineEmits<{
  'update:modelValue': [value: UploadConflictStrategy | DownloadConflictStrategy]
}>()

const uploadOptions: StrategyOption[] = [
  {
    value: 'smart_dedup',
    label: '智能去重',
    description: '比较文件内容，相同则秒传，不同则自动重命名'
  },
  {
    value: 'auto_rename',
    label: '自动重命名',
    description: '如果远程路径已存在文件则自动生成新文件名'
  },
  {
    value: 'overwrite',
    label: '覆盖',
    description: '直接覆盖远程已存在的文件（危险操作）'
  }
]

const downloadOptions: StrategyOption[] = [
  {
    value: 'overwrite',
    label: '覆盖',
    description: '如果本地文件已存在则覆盖'
  },
  {
    value: 'skip',
    label: '跳过',
    description: '如果本地文件已存在则跳过下载'
  },
  {
    value: 'auto_rename',
    label: '自动重命名',
    description: '如果本地文件已存在则自动生成新文件名'
  }
]

const options = computed(() => {
  return props.type === 'upload' ? uploadOptions : downloadOptions
})

const selectedOption = computed(() => {
  return options.value.find(opt => opt.value === props.modelValue)
})
</script>

<style scoped>
.conflict-strategy-selector {
  width: 100%;
}

.strategy-option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
}

.strategy-label {
  flex: 1;
}

.info-icon {
  color: var(--app-text-secondary);
  font-size: 14px;
  margin-left: 8px;
}

.info-icon:hover {
  color: var(--app-accent);
}

.strategy-description {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  margin-top: 8px;
  padding: 8px 12px;
  background-color: var(--app-surface-hover);
  border-radius: 8px;
  font-size: 12px;
  color: var(--app-text-secondary);
  line-height: 1.5;
}

.strategy-description .el-icon {
  color: var(--app-text-secondary);
  margin-top: 2px;
  flex-shrink: 0;
}
</style>
