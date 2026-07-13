import { flushPromises, mount } from '@vue/test-utils'
import { nextTick, ref } from 'vue'
import ElementPlus from 'element-plus'
import { describe, expect, it, vi } from 'vitest'
import FilesView from '../FilesView.vue'

const fileApiMocks = vi.hoisted(() => ({
  getFileList: vi.fn(),
}))

vi.mock('@/utils/responsive', () => ({
  useIsMobile: () => ref(false),
}))

vi.mock('@/api/file', () => ({
  getFileList: fileApiMocks.getFileList,
  searchFiles: vi.fn().mockResolvedValue({
    list: [],
    has_more: false,
  }),
  formatFileSize: vi.fn((size: number) => `${size}B`),
  formatTime: vi.fn(() => '2026-04-10 00:00:00'),
  createFolder: vi.fn(),
  deleteFiles: vi.fn(),
  copyFiles: vi.fn(),
  moveFiles: vi.fn(),
  renameFile: vi.fn(),
  validateFilename: vi.fn(() => null),
  joinPath: vi.fn((parent: string, name: string) => parent === '/' ? `/${name}` : `${parent}/${name}`),
  basename: vi.fn((path: string) => path.split('/').filter(Boolean).at(-1) || ''),
  dirname: vi.fn((path: string) => path.split('/').slice(0, -1).join('/') || '/'),
}))

vi.mock('@/api/download', () => ({
  createDownload: vi.fn(),
  createFolderDownload: vi.fn(),
  createBatchDownload: vi.fn(),
}))

vi.mock('@/api/upload', () => ({
  createUpload: vi.fn(),
  createFolderUpload: vi.fn(),
}))

vi.mock('@/api/config', () => ({
  getConfig: vi.fn().mockResolvedValue({
    download: {
      recent_directory: '',
      default_directory: '',
      download_dir: 'downloads',
      ask_each_time: false,
    },
    upload: {
      recent_directory: '',
    },
    conflict_strategy: {
      default_upload_strategy: 'smart_dedup',
      default_download_strategy: 'overwrite',
    },
  }),
  updateRecentDirDebounced: vi.fn(),
  setDefaultDownloadDir: vi.fn(),
}))

vi.mock('@/api/autobackup', () => ({
  getEncryptionStatus: vi.fn().mockResolvedValue({
    has_key: false,
  }),
}))

describe('FilesView 目录导航', () => {
  it('进入文件夹期间不会被根目录滚动分页请求覆盖', async () => {
    const folder = {
      fs_id: 1,
      path: '/folder',
      server_filename: 'folder',
      size: 0,
      isdir: 1,
      category: 6,
      server_ctime: 0,
      server_mtime: 0,
      local_ctime: 0,
      local_mtime: 0,
      is_encrypted: false,
      is_encrypted_folder: false,
    }
    let resolveFolder: ((value: { list: never[], has_more: boolean, page: number }) => void) | undefined

    fileApiMocks.getFileList
      .mockReset()
      .mockResolvedValueOnce({ list: [folder], has_more: true, page: 1 })
      .mockImplementationOnce(() => new Promise(resolve => {
        resolveFolder = resolve
      }))

    const wrapper = mount(FilesView, {
      attachTo: document.body,
      global: {
        plugins: [ElementPlus],
        stubs: {
          HomeFilled: true,
          Search: true,
          Close: true,
          Download: true,
          CopyDocument: true,
          Rank: true,
          Link: true,
          FolderAdd: true,
          Upload: true,
          Share: true,
          ArrowDown: true,
          TopRight: true,
          Edit: true,
          Delete: true,
          Refresh: true,
          Folder: true,
          Document: true,
          Loading: true,
          'el-table': true,
          'el-table-column': true,
          FilePickerModal: true,
          TransferDialog: true,
          ShareDialog: true,
          ShareDirectDownloadDialog: true,
        },
      },
    })

    await flushPromises()
    await nextTick()

    expect(fileApiMocks.getFileList).toHaveBeenCalledTimes(1)
    expect(fileApiMocks.getFileList).toHaveBeenLastCalledWith('/', 1, 50)

    wrapper.findComponent({ name: 'ElTable' }).vm.$emit('row-click', folder)
    await nextTick()

    expect(fileApiMocks.getFileList).toHaveBeenCalledTimes(2)
    expect(fileApiMocks.getFileList).toHaveBeenLastCalledWith('/folder', 1, 50)

    const list = wrapper.find('.file-list').element
    Object.defineProperties(list, {
      scrollTop: { configurable: true, value: 950 },
      scrollHeight: { configurable: true, value: 1000 },
      clientHeight: { configurable: true, value: 100 },
    })
    list.dispatchEvent(new Event('scroll'))
    await nextTick()

    // 修复前这里会发出 getFileList('/', 2, 50)，并让文件夹请求失效。
    expect(fileApiMocks.getFileList).toHaveBeenCalledTimes(2)

    resolveFolder?.({ list: [], has_more: false, page: 1 })
    await flushPromises()
    await nextTick()

    expect(wrapper.find('.path-display').text()).toBe('/folder')

    wrapper.unmount()
  })
})
