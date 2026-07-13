import { describe, expect, it } from 'vitest'
import { getParentFolder } from '../shellAdapter'

describe('getParentFolder', () => {
  it('returns the parent folder for a Windows file path', () => {
    expect(getParentFolder('D:\\Downloads\\archive.zip')).toBe('D:\\Downloads')
  })

  it('preserves a Windows drive root', () => {
    expect(getParentFolder('D:\\archive.zip')).toBe('D:\\')
  })

  it('returns the parent folder for a POSIX file path', () => {
    expect(getParentFolder('/home/user/archive.zip')).toBe('/home/user')
  })

  it('preserves the POSIX root', () => {
    expect(getParentFolder('/archive.zip')).toBe('/')
  })

  it('returns an empty string when no parent is present', () => {
    expect(getParentFolder('archive.zip')).toBe('')
  })
})
