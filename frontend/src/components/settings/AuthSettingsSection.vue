<template>
  <el-card class="setting-card auth-settings-card" shadow="hover">
    <template #header>
      <div class="card-header">
        <el-icon :size="20" color="var(--app-accent)">
          <Lock />
        </el-icon>
        <span>Web 访问认证</span>
      </div>
    </template>

    <!-- 加载状态 -->
    <el-skeleton :loading="loading" :rows="4" animated>
      <template #default>
        <!-- 当前认证状态 -->
        <div class="auth-status-card">
          <div class="status-header">
            <span class="status-label">认证状态</span>
            <el-tag :type="authConfig?.enabled ? 'success' : 'info'" size="small">
              {{ authConfig?.enabled ? '已启用' : '未启用' }}
            </el-tag>
          </div>
          <div v-if="authConfig?.enabled" class="status-detail">
            模式: {{ authModeLabel }}
            <template v-if="authConfig.password_set"> | 密码: 已设置</template>
            <template v-if="authConfig.totp_enabled"> | 2FA: 已启用</template>
          </div>
        </div>

        <!-- 认证模式切换 -->
        <el-form-item label="认证模式">
          <el-select v-model="selectedMode" style="width: 100%" @change="handleModeChange">
            <el-option value="none" label="无认证（直接访问）" />
            <el-option value="password" label="仅密码认证" />
            <el-option value="totp" label="仅双因素认证 (2FA)" />
            <el-option value="password_totp" label="密码 + 双因素认证" />
          </el-select>
          <div class="form-tip">
            选择适合您部署环境的认证方式。公网部署建议启用密码+2FA
          </div>
        </el-form-item>

        <!-- 密码设置区域 -->
        <template v-if="showPasswordSection">
          <el-divider content-position="left">密码设置</el-divider>
          <template v-if="!authConfig?.password_set">
            <el-form-item label="设置密码">
              <el-input v-model="passwordForm.newPassword" type="password" placeholder="请输入密码（至少8位）" show-password autocomplete="new-password" />
            </el-form-item>
            <el-form-item label="确认密码">
              <el-input v-model="passwordForm.confirmPassword" type="password" placeholder="请再次输入密码" show-password autocomplete="new-password" />
            </el-form-item>
            <div class="password-strength" v-if="passwordForm.newPassword">
              <span class="strength-label">密码强度:</span>
              <el-progress :percentage="passwordStrength.percentage" :status="passwordStrength.status" :stroke-width="8" style="flex: 1" />
              <span class="strength-text" :class="passwordStrength.class">{{ passwordStrength.text }}</span>
            </div>
            <el-button type="primary" @click="handleSetPassword" :loading="settingPassword">
              <el-icon><Check /></el-icon>
              设置密码
            </el-button>
          </template>
          <template v-else>
            <el-alert type="success" :closable="false" style="margin-bottom: 16px">
              <template #title><el-icon><CircleCheck /></el-icon> 密码已设置</template>
            </el-alert>
            <el-button @click="showChangePasswordDialog = true">
              <el-icon><Edit /></el-icon>
              修改密码
            </el-button>
          </template>
        </template>

        <!-- 2FA 设置区域 -->
        <template v-if="showTotpSection">
          <el-divider content-position="left">双因素认证 (2FA)</el-divider>
          <template v-if="!authConfig?.totp_enabled">
            <el-alert type="info" :closable="false" style="margin-bottom: 16px">
              <template #title>启用双因素认证可以大幅提升账户安全性</template>
            </el-alert>
            <el-button type="primary" @click="handleSetupTotp" :loading="settingUpTotp">
              <el-icon><Key /></el-icon>
              启用 2FA
            </el-button>
          </template>
          <template v-else>
            <el-alert type="success" :closable="false" style="margin-bottom: 16px">
              <template #title><el-icon><CircleCheck /></el-icon> 双因素认证已启用</template>
            </el-alert>
            <div class="totp-actions">
              <el-button @click="showRegenerateCodesDialog = true">
                <el-icon><Refresh /></el-icon>
                重新生成恢复码
              </el-button>
              <el-button type="danger" plain @click="showDisableTotpDialog = true">
                <el-icon><Close /></el-icon>
                禁用 2FA
              </el-button>
            </div>
            <div class="form-tip" style="margin-top: 8px">剩余恢复码: {{ authConfig?.recovery_codes_count || 0 }} 个</div>
          </template>
        </template>

        <!-- 配置变更警告 -->
        <el-alert v-if="hasUnsavedChanges" type="warning" :closable="false" style="margin-top: 16px">
          <template #title>修改认证配置后，所有现有会话将失效，需要重新登录</template>
        </el-alert>
      </template>
    </el-skeleton>

    <!-- 修改密码对话框 -->
    <el-dialog v-model="showChangePasswordDialog" title="修改密码" width="450px" :close-on-click-modal="false">
      <el-form @submit.prevent="handleChangePassword">
        <el-form-item label="当前密码">
          <el-input v-model="changePasswordForm.currentPassword" type="password" placeholder="请输入当前密码" show-password autocomplete="current-password" />
        </el-form-item>
        <el-form-item label="新密码">
          <el-input v-model="changePasswordForm.newPassword" type="password" placeholder="请输入新密码（至少8位）" show-password autocomplete="new-password" />
        </el-form-item>
        <el-form-item label="确认新密码">
          <el-input v-model="changePasswordForm.confirmPassword" type="password" placeholder="请再次输入新密码" show-password autocomplete="new-password" />
        </el-form-item>
        <div class="password-strength" v-if="changePasswordForm.newPassword">
          <span class="strength-label">密码强度:</span>
          <el-progress :percentage="changePasswordStrength.percentage" :status="changePasswordStrength.status" :stroke-width="8" style="flex: 1" />
          <span class="strength-text" :class="changePasswordStrength.class">{{ changePasswordStrength.text }}</span>
        </div>
      </el-form>
      <template #footer>
        <el-button @click="showChangePasswordDialog = false">取消</el-button>
        <el-button type="primary" @click="handleChangePassword" :loading="changingPassword">确认修改</el-button>
      </template>
    </el-dialog>

    <!-- TOTP 设置对话框 -->
    <el-dialog v-model="showTotpSetupDialog" title="设置双因素认证" width="500px" :close-on-click-modal="false">
      <div class="totp-setup-content">
        <el-steps :active="totpSetupStep" simple style="margin-bottom: 20px">
          <el-step title="扫描二维码" />
          <el-step title="验证" />
          <el-step title="保存恢复码" />
        </el-steps>
        <div v-if="totpSetupStep === 0" class="totp-step">
          <p class="step-hint">使用 Google Authenticator、Microsoft Authenticator 或其他 TOTP 应用扫描下方二维码</p>
          <div class="qr-code-container">
            <img v-if="totpSetupData?.qr_code" :src="totpSetupData.qr_code" alt="TOTP QR Code" class="qr-code" />
            <el-skeleton v-else :rows="0" animated style="width: 200px; height: 200px" />
          </div>
          <el-collapse>
            <el-collapse-item title="无法扫描？手动输入密钥">
              <div class="secret-key-display">
                <code>{{ totpSetupData?.secret }}</code>
                <el-button link @click="copySecret"><el-icon><CopyDocument /></el-icon></el-button>
              </div>
            </el-collapse-item>
          </el-collapse>
          <el-button type="primary" style="width: 100%; margin-top: 16px" @click="totpSetupStep = 1">下一步</el-button>
        </div>
        <div v-if="totpSetupStep === 1" class="totp-step">
          <p class="step-hint">请输入您的身份验证器应用中显示的 6 位验证码</p>
          <el-input v-model="totpVerifyCode" placeholder="000000" maxlength="6" size="large" inputmode="numeric" pattern="[0-9]*" style="text-align: center; font-size: 24px; letter-spacing: 8px" @keyup.enter="handleVerifyTotp" />
          <div class="step-actions">
            <el-button @click="totpSetupStep = 0">上一步</el-button>
            <el-button type="primary" @click="handleVerifyTotp" :loading="verifyingTotp">验证</el-button>
          </div>
        </div>
        <div v-if="totpSetupStep === 2" class="totp-step">
          <el-alert type="warning" :closable="false" style="margin-bottom: 16px">
            <template #title><strong>重要！</strong>请立即保存这些恢复码到安全的地方</template>
            <template #default>如果您丢失了身份验证器设备，可以使用恢复码登录。每个恢复码只能使用一次。</template>
          </el-alert>
          <div class="recovery-codes-display">
            <div v-for="code in recoveryCodes" :key="code" class="recovery-code-item">{{ code }}</div>
          </div>
          <div class="recovery-codes-actions">
            <el-button @click="copyRecoveryCodes"><el-icon><CopyDocument /></el-icon> 复制全部</el-button>
            <el-button @click="downloadRecoveryCodes"><el-icon><Download /></el-icon> 下载</el-button>
          </div>
          <el-button type="primary" style="width: 100%; margin-top: 16px" @click="finishTotpSetup">我已保存恢复码</el-button>
        </div>
      </div>
    </el-dialog>

    <!-- 禁用 2FA 对话框 -->
    <el-dialog v-model="showDisableTotpDialog" title="禁用双因素认证" width="400px" :close-on-click-modal="false">
      <el-alert type="error" :closable="false" style="margin-bottom: 16px">
        <template #title>禁用 2FA 会降低账户安全性</template>
      </el-alert>
      <p>请输入当前的 TOTP 验证码或恢复码来确认禁用：</p>
      <el-radio-group v-model="disableTotpMethod" style="margin-bottom: 16px">
        <el-radio value="totp">使用验证码</el-radio>
        <el-radio value="recovery">使用恢复码</el-radio>
      </el-radio-group>
      <el-input v-if="disableTotpMethod === 'totp'" v-model="disableTotpCode" placeholder="请输入 6 位验证码" maxlength="6" inputmode="numeric" pattern="[0-9]*" />
      <el-input v-else v-model="disableRecoveryCode" placeholder="请输入恢复码 (XXXX-XXXX)" />
      <template #footer>
        <el-button @click="showDisableTotpDialog = false">取消</el-button>
        <el-button type="danger" @click="handleDisableTotp" :loading="disablingTotp">确认禁用</el-button>
      </template>
    </el-dialog>

    <!-- 重新生成恢复码对话框 -->
    <el-dialog v-model="showRegenerateCodesDialog" title="重新生成恢复码" width="450px" :close-on-click-modal="false">
      <div v-if="!regeneratedCodes">
        <el-alert type="warning" :closable="false" style="margin-bottom: 16px">
          <template #title>重新生成将使所有旧恢复码失效</template>
        </el-alert>
        <p>请输入当前的 TOTP 验证码来确认：</p>
        <el-input v-model="regenerateVerifyCode" placeholder="请输入 6 位验证码" maxlength="6" inputmode="numeric" pattern="[0-9]*" @keyup.enter="handleRegenerateCodes" />
      </div>
      <div v-else>
        <el-alert type="success" :closable="false" style="margin-bottom: 16px">
          <template #title>新的恢复码已生成</template>
        </el-alert>
        <div class="recovery-codes-display">
          <div v-for="code in regeneratedCodes" :key="code" class="recovery-code-item">{{ code }}</div>
        </div>
        <div class="recovery-codes-actions">
          <el-button @click="copyRegeneratedCodes"><el-icon><CopyDocument /></el-icon> 复制全部</el-button>
          <el-button @click="downloadRegeneratedCodes"><el-icon><Download /></el-icon> 下载</el-button>
        </div>
      </div>
      <template #footer>
        <template v-if="!regeneratedCodes">
          <el-button @click="showRegenerateCodesDialog = false">取消</el-button>
          <el-button type="primary" @click="handleRegenerateCodes" :loading="regeneratingCodes">确认重新生成</el-button>
        </template>
        <template v-else>
          <el-button type="primary" @click="finishRegenerateCodes">我已保存恢复码</el-button>
        </template>
      </template>
    </el-dialog>
  </el-card>
</template>
<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { webAuthApi, type AuthConfigResponse, type AuthMode, type TotpSetupResponse } from '@/api/webAuth'
import { Lock, Key, Check, Edit, Refresh, Close, CircleCheck, CopyDocument, Download } from '@element-plus/icons-vue'

const loading = ref(false)
const authConfig = ref<AuthConfigResponse | null>(null)
const selectedMode = ref<AuthMode>('none')
const originalMode = ref<AuthMode>('none')
const passwordForm = ref({ newPassword: '', confirmPassword: '' })
const settingPassword = ref(false)
const showChangePasswordDialog = ref(false)
const changePasswordForm = ref({ currentPassword: '', newPassword: '', confirmPassword: '' })
const changingPassword = ref(false)
const showTotpSetupDialog = ref(false)
const totpSetupStep = ref(0)
const totpSetupData = ref<TotpSetupResponse | null>(null)
const totpVerifyCode = ref('')
const settingUpTotp = ref(false)
const verifyingTotp = ref(false)
const recoveryCodes = ref<string[]>([])
const showDisableTotpDialog = ref(false)
const disableTotpMethod = ref<'totp' | 'recovery'>('totp')
const disableTotpCode = ref('')
const disableRecoveryCode = ref('')
const disablingTotp = ref(false)
const showRegenerateCodesDialog = ref(false)
const regenerateVerifyCode = ref('')
const regeneratingCodes = ref(false)
const regeneratedCodes = ref<string[] | null>(null)

const hasUnsavedChanges = computed(() => selectedMode.value !== originalMode.value)
const showPasswordSection = computed(() => selectedMode.value === 'password' || selectedMode.value === 'password_totp')
const showTotpSection = computed(() => selectedMode.value === 'totp' || selectedMode.value === 'password_totp')
const authModeLabel = computed(() => {
  const labels: Record<AuthMode, string> = { none: '无认证', password: '仅密码', totp: '仅 2FA', password_totp: '密码 + 2FA' }
  return labels[authConfig.value?.mode || 'none']
})

function calculatePasswordStrength(password: string) {
  if (!password) return { percentage: 0, status: '' as const, text: '', class: '' }
  let score = 0
  if (password.length >= 8) score += 25
  if (password.length >= 12) score += 15
  if (/[a-z]/.test(password)) score += 15
  if (/[A-Z]/.test(password)) score += 15
  if (/[0-9]/.test(password)) score += 15
  if (/[^a-zA-Z0-9]/.test(password)) score += 15
  if (score < 40) return { percentage: score, status: 'exception' as const, text: '弱', class: 'weak' }
  if (score < 70) return { percentage: score, status: 'warning' as const, text: '中', class: 'medium' }
  return { percentage: score, status: 'success' as const, text: '强', class: 'strong' }
}

const passwordStrength = computed(() => calculatePasswordStrength(passwordForm.value.newPassword))
const changePasswordStrength = computed(() => calculatePasswordStrength(changePasswordForm.value.newPassword))

async function loadAuthConfig() {
  loading.value = true
  try {
    authConfig.value = await webAuthApi.getConfig()
    selectedMode.value = authConfig.value.mode
    originalMode.value = authConfig.value.mode
  } catch (error: any) {
    ElMessage.error('加载认证配置失败: ' + (error.message || '未知错误'))
  } finally {
    loading.value = false
  }
}

async function handleModeChange(newMode: AuthMode) {
  if ((newMode === 'password' || newMode === 'password_totp') && !authConfig.value?.password_set) {
    ElMessage.warning('请先设置密码')
    return
  }
  if ((newMode === 'totp' || newMode === 'password_totp') && !authConfig.value?.totp_enabled) {
    ElMessage.warning('请先启用双因素认证')
    return
  }
  try {
    await ElMessageBox.confirm('修改认证模式后，所有现有会话将失效，需要重新登录。确定要继续吗？', '确认修改', { confirmButtonText: '确定', cancelButtonText: '取消', type: 'warning' })
    await webAuthApi.updateConfig({ mode: newMode, enabled: newMode !== 'none' })
    ElMessage.success('认证模式已更新，即将跳转到登录页')
    // 清除本地令牌和配置缓存
    localStorage.removeItem('web_auth_access_token')
    localStorage.removeItem('web_auth_refresh_token')
    localStorage.removeItem('web_auth_access_expires_at')
    localStorage.removeItem('web_auth_refresh_expires_at')
    localStorage.removeItem('web_auth_config')
    localStorage.removeItem('web_auth_config_time')
    // 延迟跳转，让用户看到提示
    setTimeout(() => {
      window.location.hash = '#/web-login'
    }, 1000)
  } catch (error: any) {
    if (error !== 'cancel') ElMessage.error('更新认证模式失败: ' + (error.message || '未知错误'))
    selectedMode.value = originalMode.value
  }
}

async function handleSetPassword() {
  if (!passwordForm.value.newPassword) { ElMessage.warning('请输入密码'); return }
  if (passwordForm.value.newPassword.length < 8) { ElMessage.warning('密码长度至少为 8 位'); return }
  if (passwordForm.value.newPassword !== passwordForm.value.confirmPassword) { ElMessage.warning('两次输入的密码不一致'); return }
  settingPassword.value = true
  try {
    await webAuthApi.setPassword({ password: passwordForm.value.newPassword })
    ElMessage.success('密码设置成功')
    passwordForm.value = { newPassword: '', confirmPassword: '' }
    await loadAuthConfig()
    // 如果当前选择的模式需要密码，提示用户可以切换模式了
    if (selectedMode.value === 'password' || selectedMode.value === 'password_totp') {
      ElMessage.info('密码已设置，您现在可以启用密码认证模式')
    }
  } catch (error: any) {
    ElMessage.error('设置密码失败: ' + (error.message || '未知错误'))
  } finally {
    settingPassword.value = false
  }
}

async function handleChangePassword() {
  if (!changePasswordForm.value.currentPassword) { ElMessage.warning('请输入当前密码'); return }
  if (!changePasswordForm.value.newPassword) { ElMessage.warning('请输入新密码'); return }
  if (changePasswordForm.value.newPassword.length < 8) { ElMessage.warning('新密码长度至少为 8 位'); return }
  if (changePasswordForm.value.newPassword !== changePasswordForm.value.confirmPassword) { ElMessage.warning('两次输入的密码不一致'); return }
  changingPassword.value = true
  try {
    await webAuthApi.setPassword({ password: changePasswordForm.value.newPassword, current_password: changePasswordForm.value.currentPassword })
    ElMessage.success('密码修改成功')
    showChangePasswordDialog.value = false
    changePasswordForm.value = { currentPassword: '', newPassword: '', confirmPassword: '' }
  } catch (error: any) {
    ElMessage.error('修改密码失败: ' + (error.message || '未知错误'))
  } finally {
    changingPassword.value = false
  }
}

async function handleSetupTotp() {
  settingUpTotp.value = true
  try {
    totpSetupData.value = await webAuthApi.setupTotp()
    totpSetupStep.value = 0
    totpVerifyCode.value = ''
    recoveryCodes.value = []
    showTotpSetupDialog.value = true
  } catch (error: any) {
    ElMessage.error('获取 TOTP 设置信息失败: ' + (error.message || '未知错误'))
  } finally {
    settingUpTotp.value = false
  }
}

async function handleVerifyTotp() {
  if (!totpVerifyCode.value || totpVerifyCode.value.length !== 6) { ElMessage.warning('请输入 6 位验证码'); return }
  verifyingTotp.value = true
  try {
    // 传递 secret 参数，确保后端能正确验证
    await webAuthApi.verifyTotp({ code: totpVerifyCode.value, secret: totpSetupData.value?.secret })
    const response = await webAuthApi.regenerateRecoveryCodes({ totp_code: totpVerifyCode.value })
    recoveryCodes.value = response.codes
    totpSetupStep.value = 2
    ElMessage.success('双因素认证已启用')
  } catch (error: any) {
    ElMessage.error('验证失败: ' + (error.message || '未知错误'))
    totpVerifyCode.value = ''
  } finally {
    verifyingTotp.value = false
  }
}

function finishTotpSetup() {
  showTotpSetupDialog.value = false
  totpSetupData.value = null
  totpSetupStep.value = 0
  totpVerifyCode.value = ''
  recoveryCodes.value = []
  loadAuthConfig()
}

function copySecret() {
  if (totpSetupData.value?.secret) {
    navigator.clipboard.writeText(totpSetupData.value.secret)
    ElMessage.success('密钥已复制到剪贴板')
  }
}

function copyRecoveryCodes() {
  navigator.clipboard.writeText(recoveryCodes.value.join('\n'))
  ElMessage.success('恢复码已复制到剪贴板')
}

function downloadRecoveryCodes() {
  const text = `BaiduPCS-Rust Web 认证恢复码\n生成时间: ${new Date().toLocaleString()}\n\n${recoveryCodes.value.join('\n')}\n\n注意: 每个恢复码只能使用一次，请妥善保管！`
  const blob = new Blob([text], { type: 'text/plain' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = 'recovery-codes.txt'
  a.click()
  URL.revokeObjectURL(url)
  ElMessage.success('恢复码已下载')
}

async function handleDisableTotp() {
  const code = disableTotpMethod.value === 'totp' ? disableTotpCode.value : undefined
  const recoveryCode = disableTotpMethod.value === 'recovery' ? disableRecoveryCode.value : undefined
  if (!code && !recoveryCode) { ElMessage.warning('请输入验证码或恢复码'); return }
  disablingTotp.value = true
  try {
    await webAuthApi.disableTotp({ code, recovery_code: recoveryCode })
    ElMessage.success('双因素认证已禁用')
    showDisableTotpDialog.value = false
    disableTotpCode.value = ''
    disableRecoveryCode.value = ''
    await loadAuthConfig()
  } catch (error: any) {
    ElMessage.error('禁用失败: ' + (error.message || '未知错误'))
  } finally {
    disablingTotp.value = false
  }
}

async function handleRegenerateCodes() {
  if (!regenerateVerifyCode.value || regenerateVerifyCode.value.length !== 6) { ElMessage.warning('请输入 6 位验证码'); return }
  regeneratingCodes.value = true
  try {
    const response = await webAuthApi.regenerateRecoveryCodes({ totp_code: regenerateVerifyCode.value })
    regeneratedCodes.value = response.codes
    ElMessage.success('恢复码已重新生成')
    await loadAuthConfig()
  } catch (error: any) {
    ElMessage.error('重新生成失败: ' + (error.message || '未知错误'))
  } finally {
    regeneratingCodes.value = false
  }
}

function copyRegeneratedCodes() {
  if (regeneratedCodes.value) {
    navigator.clipboard.writeText(regeneratedCodes.value.join('\n'))
    ElMessage.success('恢复码已复制到剪贴板')
  }
}

function downloadRegeneratedCodes() {
  if (regeneratedCodes.value) {
    const text = `BaiduPCS-Rust Web 认证恢复码\n生成时间: ${new Date().toLocaleString()}\n\n${regeneratedCodes.value.join('\n')}\n\n注意: 每个恢复码只能使用一次，请妥善保管！`
    const blob = new Blob([text], { type: 'text/plain' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = 'recovery-codes.txt'
    a.click()
    URL.revokeObjectURL(url)
    ElMessage.success('恢复码已下载')
  }
}

function finishRegenerateCodes() {
  showRegenerateCodesDialog.value = false
  regenerateVerifyCode.value = ''
  regeneratedCodes.value = null
}

watch(showChangePasswordDialog, (val) => { if (!val) changePasswordForm.value = { currentPassword: '', newPassword: '', confirmPassword: '' } })
watch(showDisableTotpDialog, (val) => { if (!val) { disableTotpCode.value = ''; disableRecoveryCode.value = ''; disableTotpMethod.value = 'totp' } })
watch(showRegenerateCodesDialog, (val) => { if (!val) { regenerateVerifyCode.value = ''; regeneratedCodes.value = null } })

onMounted(() => { loadAuthConfig() })
</script>
<style scoped lang="scss">
.auth-settings-card {
  .card-header { display: flex; align-items: center; gap: 8px; font-size: 16px; font-weight: 600; color: var(--app-text-primary); }
}
.auth-status-card {
  background: var(--app-surface-hover); border-radius: 8px; padding: 16px; margin-bottom: 16px;
  .status-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; .status-label { font-size: 14px; font-weight: 500; } }
  .status-detail { font-size: 13px; color: var(--app-text-secondary); }
}
.form-tip { margin-top: 4px; font-size: 12px; color: var(--app-text-secondary); line-height: 1.5; }
.password-strength {
  display: flex; align-items: center; gap: 12px; margin-bottom: 16px;
  .strength-label { font-size: 13px; color: var(--app-text-secondary); white-space: nowrap; }
  .strength-text { font-size: 13px; font-weight: 500; white-space: nowrap; &.weak { color: var(--el-color-danger); } &.medium { color: var(--el-color-warning); } &.strong { color: var(--el-color-success); } }
}
.totp-actions { display: flex; gap: 12px; flex-wrap: wrap; }
.totp-setup-content {
  .totp-step {
    .step-hint { font-size: 14px; color: var(--app-text-secondary); margin-bottom: 16px; line-height: 1.6; }
    .step-actions { display: flex; justify-content: flex-end; gap: 12px; margin-top: 16px; }
  }
  .qr-code-container { display: flex; justify-content: center; margin: 20px 0; .qr-code { width: 200px; height: 200px; border: 1px solid var(--app-divider); border-radius: 8px; } }
  .secret-key-display { display: flex; align-items: center; gap: 8px; background: var(--app-surface-hover); padding: 12px; border-radius: 8px; font-family: monospace; word-break: break-all; code { flex: 1; font-size: 14px; } }
}
.recovery-codes-display {
  display: grid; grid-template-columns: repeat(2, 1fr); gap: 8px; margin-bottom: 16px;
  .recovery-code-item { background: var(--app-surface-hover); padding: 10px 12px; border-radius: 8px; font-family: monospace; font-size: 14px; text-align: center; border: 1px solid var(--app-divider); }
}
.recovery-codes-actions { display: flex; justify-content: center; gap: 12px; }
:deep(.el-dialog) {
  border-radius: 14px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.12);
  .el-dialog__header { border-bottom: 1px solid var(--app-divider); }
  .el-dialog__footer { border-top: 1px solid var(--app-divider); }
}
</style>
