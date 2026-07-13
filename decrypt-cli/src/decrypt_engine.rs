//! 解密引擎模块
//!
//! 实现文件解密的核心逻辑，支持 AES-256-GCM 和 ChaCha20-Poly1305 两种算法。
//!
//! # 功能
//!
//! - 单文件解密：根据文件头自动选择解密算法
//! - 批量解密：递归遍历目录，根据映射记录恢复原始目录结构
//! - 流式分块解密：支持大文件处理

use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce as AesNonce,
};
use base64::Engine;
use chacha20poly1305::{ChaCha20Poly1305, Nonce as ChaChaPolyNonce};

use crate::file_parser::{ChunkReader, FileHeaderParser};
use crate::key_loader::KeyLoader;
use crate::mapping_loader::MappingLoader;
use crate::types::{DecryptError, DecryptSummary, EncryptionAlgorithm, EncryptionKeyInfo};

/// 解密引擎
///
/// 提供文件解密功能，支持单文件和批量解密模式。
pub struct DecryptEngine;

impl DecryptEngine {
    /// 创建新的解密引擎
    pub fn new() -> Self {
        Self
    }

    /// 解密单个文件
    ///
    /// 根据文件头中的 algorithm 字段自动选择解密算法。
    ///
    /// # Arguments
    /// * `input` - 输入文件路径（加密文件）
    /// * `output` - 输出文件路径（解密后的文件）
    /// * `key` - 解密密钥
    ///
    /// # Returns
    /// * `Ok(())` - 解密成功
    /// * `Err(DecryptError)` - 解密失败
    pub fn decrypt_file(
        &self,
        input: &Path,
        output: &Path,
        key: &EncryptionKeyInfo,
    ) -> Result<(), DecryptError> {
        // 打开输入文件
        let file = File::open(input)?;
        let mut reader = BufReader::new(file);

        // 解析文件头
        let header = FileHeaderParser::parse(&mut reader)?;

        // 验证 magic
        if !header.validate_magic() {
            return Err(DecryptError::InvalidFormat(
                "文件魔数不匹配".to_string(),
            ));
        }

        // 解码密钥
        let key_bytes = base64::engine::general_purpose::STANDARD
            .decode(&key.master_key)
            .map_err(|e| DecryptError::KeyDecodeError(format!("Base64 解码失败: {}", e)))?;

        if key_bytes.len() != 32 {
            return Err(DecryptError::KeyDecodeError(format!(
                "密钥长度错误: 期望 32 字节，实际 {} 字节",
                key_bytes.len()
            )));
        }

        // 创建输出目录（如果不存在）
        if let Some(parent) = output.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        // 创建输出文件
        let out_file = File::create(output)?;
        let mut writer = BufWriter::new(out_file);

        // 创建分块读取器
        let chunk_reader = ChunkReader::new(reader, header.total_chunks);

        // 根据算法选择解密方式
        match header.algorithm {
            EncryptionAlgorithm::Aes256Gcm => {
                self.decrypt_chunks_aes(&key_bytes, chunk_reader, &mut writer)?;
            }
            EncryptionAlgorithm::ChaCha20Poly1305 => {
                self.decrypt_chunks_chacha(&key_bytes, chunk_reader, &mut writer)?;
            }
        }

        writer.flush()?;
        Ok(())
    }

    /// 使用 AES-256-GCM 解密分块
    fn decrypt_chunks_aes<R: std::io::Read, W: Write>(
        &self,
        key_bytes: &[u8],
        chunk_reader: ChunkReader<R>,
        writer: &mut W,
    ) -> Result<(), DecryptError> {
        let key: [u8; 32] = key_bytes
            .try_into()
            .map_err(|_| DecryptError::KeyDecodeError("密钥长度错误".to_string()))?;

        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| DecryptError::KeyDecodeError(format!("创建 AES 密码器失败: {}", e)))?;

        for (chunk_index, chunk_result) in chunk_reader.enumerate() {
            let chunk = chunk_result?;
            let nonce = AesNonce::from_slice(&chunk.nonce);

            let plaintext = cipher
                .decrypt(nonce, chunk.ciphertext.as_ref())
                .map_err(|_| {
                    DecryptError::DecryptionFailed(format!(
                        "AES-256-GCM 解密分块 {} 失败（可能是密钥不匹配或数据损坏）",
                        chunk_index
                    ))
                })?;

            writer.write_all(&plaintext)?;
        }

        Ok(())
    }

    /// 使用 ChaCha20-Poly1305 解密分块
    fn decrypt_chunks_chacha<R: std::io::Read, W: Write>(
        &self,
        key_bytes: &[u8],
        chunk_reader: ChunkReader<R>,
        writer: &mut W,
    ) -> Result<(), DecryptError> {
        let key: [u8; 32] = key_bytes
            .try_into()
            .map_err(|_| DecryptError::KeyDecodeError("密钥长度错误".to_string()))?;

        let cipher = ChaCha20Poly1305::new_from_slice(&key).map_err(|e| {
            DecryptError::KeyDecodeError(format!("创建 ChaCha20 密码器失败: {}", e))
        })?;

        for (chunk_index, chunk_result) in chunk_reader.enumerate() {
            let chunk = chunk_result?;
            let nonce = ChaChaPolyNonce::from_slice(&chunk.nonce);

            let plaintext = cipher
                .decrypt(nonce, chunk.ciphertext.as_ref())
                .map_err(|_| {
                    DecryptError::DecryptionFailed(format!(
                        "ChaCha20-Poly1305 解密分块 {} 失败（可能是密钥不匹配或数据损坏）",
                        chunk_index
                    ))
                })?;

            writer.write_all(&plaintext)?;
        }

        Ok(())
    }

    /// 尝试使用多个密钥解密文件
    ///
    /// 用于单文件模式，遍历所有可用密钥尝试解密。
    ///
    /// # Arguments
    /// * `input` - 输入文件路径
    /// * `output` - 输出文件路径
    /// * `keys` - 所有可用密钥
    ///
    /// # Returns
    /// * `Ok(())` - 解密成功
    /// * `Err(DecryptError)` - 所有密钥都失败
    pub fn decrypt_file_with_any_key(
        &self,
        input: &Path,
        output: &Path,
        keys: &[&EncryptionKeyInfo],
    ) -> Result<(), DecryptError> {
        if keys.is_empty() {
            return Err(DecryptError::KeyMismatch(0));
        }

        let mut last_error = None;

        for key in keys {
            match self.decrypt_file(input, output, key) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    // 如果是解密失败（密钥不匹配），继续尝试下一个密钥
                    // 如果是其他错误（如文件格式错误），直接返回
                    match &e {
                        DecryptError::DecryptionFailed(_) => {
                            last_error = Some(e);
                            continue;
                        }
                        _ => return Err(e),
                    }
                }
            }
        }

        // 所有密钥都失败
        Err(last_error.unwrap_or_else(|| {
            DecryptError::KeyMismatch(0)
        }))
    }

    /// 批量解密目录
    ///
    /// 递归遍历输入目录，根据映射记录构建输出路径，自动创建目录层级。
    ///
    /// # Arguments
    /// * `in_dir` - 输入目录
    /// * `out_dir` - 输出目录
    /// * `mapping` - 映射加载器
    /// * `key_loader` - 密钥加载器
    /// * `mirror` - 是否镜像输入目录结构
    ///
    /// # Returns
    /// 解密结果汇总
    pub fn decrypt_directory(
        &self,
        in_dir: &Path,
        out_dir: &Path,
        mapping: &MappingLoader,
        key_loader: &KeyLoader,
        mirror: bool,
    ) -> DecryptSummary {
        let mut summary = DecryptSummary::new();
        // 用于跟踪已使用的文件名（扁平模式下处理同名文件）
        let mut used_names: std::collections::HashSet<String> = std::collections::HashSet::new();

        // 递归遍历输入目录
        if let Err(e) = self.process_directory(in_dir, in_dir, out_dir, mapping, key_loader, &mut summary, mirror, &mut used_names) {
            summary.add_failed(
                in_dir.display().to_string(),
                format!("遍历目录失败: {}", e),
            );
        }

        summary
    }

    /// 递归处理目录
    fn process_directory(
        &self,
        dir: &Path,
        in_dir: &Path,
        out_dir: &Path,
        mapping: &MappingLoader,
        key_loader: &KeyLoader,
        summary: &mut DecryptSummary,
        mirror: bool,
        used_names: &mut std::collections::HashSet<String>,
    ) -> Result<(), DecryptError> {
        let entries = fs::read_dir(dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // 递归处理子目录
                self.process_directory(&path, in_dir, out_dir, mapping, key_loader, summary, mirror, used_names)?;
            } else if path.is_file() {
                // 处理文件
                self.process_file(&path, in_dir, out_dir, mapping, key_loader, summary, mirror, used_names);
            }
        }

        Ok(())
    }

    /// 处理单个文件
    fn process_file(
        &self,
        input: &Path,
        in_dir: &Path,
        out_dir: &Path,
        mapping: &MappingLoader,
        key_loader: &KeyLoader,
        summary: &mut DecryptSummary,
        mirror: bool,
        used_names: &mut std::collections::HashSet<String>,
    ) {
        let input_str = input.display().to_string();

        // 获取文件名
        let file_name = match input.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => {
                summary.add_skipped(input_str, "无法获取文件名".to_string());
                return;
            }
        };

        // 检查是否为加密文件
        match FileHeaderParser::is_encrypted_file(input) {
            Ok(true) => {}
            Ok(false) => {
                summary.add_skipped(input_str, "不是加密文件".to_string());
                return;
            }
            Err(e) => {
                summary.add_failed(input_str, format!("检查文件格式失败: {}", e));
                return;
            }
        }

        // 在映射中查找记录
        let record = match mapping.find_by_encrypted_name(file_name) {
            Some(r) => r,
            None => {
                summary.add_skipped(input_str, "映射中找不到记录".to_string());
                return;
            }
        };

        // 获取密钥
        let key = match key_loader.get_key(record.key_version) {
            Some(k) => k,
            None => {
                summary.add_failed(
                    input_str,
                    format!("密钥版本 {} 不存在", record.key_version),
                );
                return;
            }
        };

        // 构建输出路径
        let output_path = if mirror {
            // 镜像模式：使用输入文件相对于 in_dir 的路径
            self.build_mirror_output_path(input, in_dir, out_dir, &record.original_name, used_names)
        } else {
            // 默认模式：使用映射中的 original_path
            self.build_output_path(out_dir, &record.original_path, &record.original_name)
        };

        // 解密文件
        match self.decrypt_file(input, &output_path, key) {
            Ok(()) => {
                summary.add_success(input_str, output_path.display().to_string());
            }
            Err(e) => {
                summary.add_failed(input_str, format!("{}", e));
            }
        }
    }

    /// 构建输出路径
    ///
    /// 根据 original_path 和 original_name 构建完整的输出路径。
    fn build_output_path(&self, out_dir: &Path, original_path: &str, original_name: &str) -> std::path::PathBuf {
        // 移除 original_path 开头的斜杠
        let clean_path = original_path.trim_start_matches('/').trim_start_matches('\\');

        if clean_path.is_empty() {
            out_dir.join(original_name)
        } else {
            out_dir.join(clean_path).join(original_name)
        }
    }

    /// 构建镜像模式输出路径
    ///
    /// 使用输入文件相对于 in_dir 的路径，而不是映射中的 original_path。
    /// 同名文件自动添加 (1)、(2) 等后缀。
    fn build_mirror_output_path(
        &self,
        input: &Path,
        in_dir: &Path,
        out_dir: &Path,
        original_name: &str,
        used_names: &mut std::collections::HashSet<String>,
    ) -> std::path::PathBuf {
        // 计算输入文件相对于 in_dir 的父目录路径
        let relative_dir = input
            .parent()
            .and_then(|p| p.strip_prefix(in_dir).ok())
            .unwrap_or(Path::new(""));

        // 构建基础输出目录
        let base_out_dir = out_dir.join(relative_dir);

        // 处理同名文件
        let final_name = self.get_unique_name(original_name, &base_out_dir, used_names);

        base_out_dir.join(&final_name)
    }

    /// 获取唯一文件名
    ///
    /// 如果文件名已存在，添加 (1)、(2) 等后缀
    fn get_unique_name(
        &self,
        original_name: &str,
        out_dir: &Path,
        used_names: &mut std::collections::HashSet<String>,
    ) -> String {
        let full_path_key = out_dir.join(original_name).display().to_string();

        if !used_names.contains(&full_path_key) {
            used_names.insert(full_path_key);
            return original_name.to_string();
        }

        // 分离文件名和扩展名
        let (stem, ext) = match original_name.rfind('.') {
            Some(pos) => (&original_name[..pos], &original_name[pos..]),
            None => (original_name, ""),
        };

        // 尝试添加 (1)、(2) 等后缀
        let mut counter = 1;
        loop {
            let new_name = format!("{}({}){}", stem, counter, ext);
            let new_path_key = out_dir.join(&new_name).display().to_string();

            if !used_names.contains(&new_path_key) {
                used_names.insert(new_path_key);
                return new_name;
            }
            counter += 1;
        }
    }
}

impl Default for DecryptEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 加密辅助函数（用于测试）
// ============================================================================

/// 使用 AES-256-GCM 加密数据（用于测试）
#[cfg(test)]
pub fn encrypt_aes_gcm(plaintext: &[u8], key: &[u8; 32], nonce: &[u8; 12]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| format!("创建 AES 密码器失败: {}", e))?;
    let nonce = AesNonce::from_slice(nonce);
    cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("AES 加密失败: {}", e))
}

/// 使用 ChaCha20-Poly1305 加密数据（用于测试）
#[cfg(test)]
pub fn encrypt_chacha20_poly1305(plaintext: &[u8], key: &[u8; 32], nonce: &[u8; 12]) -> Result<Vec<u8>, String> {
    let cipher = ChaCha20Poly1305::new_from_slice(key)
        .map_err(|e| format!("创建 ChaCha20 密码器失败: {}", e))?;
    let nonce = ChaChaPolyNonce::from_slice(nonce);
    cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("ChaCha20 加密失败: {}", e))
}

/// 使用 AES-256-GCM 解密数据（用于测试）
#[cfg(test)]
pub fn decrypt_aes_gcm(ciphertext: &[u8], key: &[u8; 32], nonce: &[u8; 12]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| format!("创建 AES 密码器失败: {}", e))?;
    let nonce = AesNonce::from_slice(nonce);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("AES 解密失败: {}", e))
}

/// 使用 ChaCha20-Poly1305 解密数据（用于测试）
#[cfg(test)]
pub fn decrypt_chacha20_poly1305(ciphertext: &[u8], key: &[u8; 32], nonce: &[u8; 12]) -> Result<Vec<u8>, String> {
    let cipher = ChaCha20Poly1305::new_from_slice(key)
        .map_err(|e| format!("创建 ChaCha20 密码器失败: {}", e))?;
    let nonce = ChaChaPolyNonce::from_slice(nonce);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("ChaCha20 解密失败: {}", e))
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{EncryptionAlgorithm, FILE_MAGIC};
    use std::io::Write;
    use tempfile::TempDir;

    /// 创建测试用的密钥信息
    fn create_test_key(key_bytes: &[u8; 32], version: u32) -> EncryptionKeyInfo {
        EncryptionKeyInfo {
            master_key: base64::engine::general_purpose::STANDARD.encode(key_bytes),
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            key_version: version,
            created_at: 1702454400000,
            last_used_at: None,
            deprecated_at: None,
        }
    }

    /// 创建加密文件（用于测试）
    fn create_encrypted_file(
        path: &Path,
        plaintext: &[u8],
        key: &[u8; 32],
        algorithm: EncryptionAlgorithm,
    ) -> std::io::Result<()> {
        let mut file = File::create(path)?;

        // 写入文件头
        file.write_all(&FILE_MAGIC)?;
        file.write_all(&[algorithm.to_byte()])?;

        // master_nonce
        let master_nonce = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C];
        file.write_all(&master_nonce)?;

        // original_size
        file.write_all(&(plaintext.len() as u64).to_le_bytes())?;

        // total_chunks (单个分块)
        file.write_all(&1u32.to_le_bytes())?;

        // 加密数据
        let chunk_nonce = master_nonce;
        let ciphertext = match algorithm {
            EncryptionAlgorithm::Aes256Gcm => {
                encrypt_aes_gcm(plaintext, key, &chunk_nonce).unwrap()
            }
            EncryptionAlgorithm::ChaCha20Poly1305 => {
                encrypt_chacha20_poly1305(plaintext, key, &chunk_nonce).unwrap()
            }
        };

        // 写入分块
        file.write_all(&chunk_nonce)?;
        file.write_all(&(ciphertext.len() as u32).to_le_bytes())?;
        file.write_all(&ciphertext)?;

        Ok(())
    }

    #[test]
    fn test_decrypt_file_aes() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("encrypted.dat");
        let output_path = temp_dir.path().join("decrypted.txt");

        let key = [0x42u8; 32];
        let plaintext = b"Hello, World! This is a test message.";

        // 创建加密文件
        create_encrypted_file(&input_path, plaintext, &key, EncryptionAlgorithm::Aes256Gcm).unwrap();

        // 解密
        let engine = DecryptEngine::new();
        let key_info = create_test_key(&key, 1);
        engine.decrypt_file(&input_path, &output_path, &key_info).unwrap();

        // 验证
        let decrypted = fs::read(&output_path).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_file_chacha() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("encrypted.dat");
        let output_path = temp_dir.path().join("decrypted.txt");

        let key = [0x42u8; 32];
        let plaintext = b"Hello, ChaCha20! This is a test message.";

        // 创建加密文件
        create_encrypted_file(&input_path, plaintext, &key, EncryptionAlgorithm::ChaCha20Poly1305).unwrap();

        // 解密
        let engine = DecryptEngine::new();
        let key_info = create_test_key(&key, 1);
        engine.decrypt_file(&input_path, &output_path, &key_info).unwrap();

        // 验证
        let decrypted = fs::read(&output_path).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_file_wrong_key() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("encrypted.dat");
        let output_path = temp_dir.path().join("decrypted.txt");

        let correct_key = [0x42u8; 32];
        let wrong_key = [0x43u8; 32];
        let plaintext = b"Secret message";

        // 使用正确密钥创建加密文件
        create_encrypted_file(&input_path, plaintext, &correct_key, EncryptionAlgorithm::Aes256Gcm).unwrap();

        // 使用错误密钥解密
        let engine = DecryptEngine::new();
        let key_info = create_test_key(&wrong_key, 1);
        let result = engine.decrypt_file(&input_path, &output_path, &key_info);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DecryptError::DecryptionFailed(_)));
    }

    #[test]
    fn test_decrypt_file_invalid_magic() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("invalid.dat");
        let output_path = temp_dir.path().join("decrypted.txt");

        // 创建无效文件
        let mut file = File::create(&input_path).unwrap();
        file.write_all(&[0x00; 31]).unwrap(); // 无效的 magic

        let engine = DecryptEngine::new();
        let key = [0x42u8; 32];
        let key_info = create_test_key(&key, 1);
        let result = engine.decrypt_file(&input_path, &output_path, &key_info);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DecryptError::InvalidFormat(_)));
    }

    #[test]
    fn test_decrypt_file_with_any_key() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("encrypted.dat");
        let output_path = temp_dir.path().join("decrypted.txt");

        let correct_key = [0x42u8; 32];
        let wrong_key1 = [0x41u8; 32];
        let wrong_key2 = [0x43u8; 32];
        let plaintext = b"Secret message";

        // 创建加密文件
        create_encrypted_file(&input_path, plaintext, &correct_key, EncryptionAlgorithm::Aes256Gcm).unwrap();

        // 使用多个密钥尝试解密
        let engine = DecryptEngine::new();
        let key_info1 = create_test_key(&wrong_key1, 1);
        let key_info2 = create_test_key(&correct_key, 2);
        let key_info3 = create_test_key(&wrong_key2, 3);

        let keys: Vec<&EncryptionKeyInfo> = vec![&key_info1, &key_info2, &key_info3];
        engine.decrypt_file_with_any_key(&input_path, &output_path, &keys).unwrap();

        // 验证
        let decrypted = fs::read(&output_path).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_file_with_any_key_all_fail() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("encrypted.dat");
        let output_path = temp_dir.path().join("decrypted.txt");

        let correct_key = [0x42u8; 32];
        let wrong_key1 = [0x41u8; 32];
        let wrong_key2 = [0x43u8; 32];
        let plaintext = b"Secret message";

        // 创建加密文件
        create_encrypted_file(&input_path, plaintext, &correct_key, EncryptionAlgorithm::Aes256Gcm).unwrap();

        // 使用错误密钥尝试解密
        let engine = DecryptEngine::new();
        let key_info1 = create_test_key(&wrong_key1, 1);
        let key_info2 = create_test_key(&wrong_key2, 2);

        let keys: Vec<&EncryptionKeyInfo> = vec![&key_info1, &key_info2];
        let result = engine.decrypt_file_with_any_key(&input_path, &output_path, &keys);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DecryptError::DecryptionFailed(_)));
    }

    #[test]
    fn test_build_output_path() {
        let engine = DecryptEngine::new();
        let out_dir = Path::new("/output");

        // 测试正常路径
        let path = engine.build_output_path(out_dir, "/documents/work", "report.pdf");
        assert_eq!(path, Path::new("/output/documents/work/report.pdf"));

        // 测试根路径
        let path = engine.build_output_path(out_dir, "/", "file.txt");
        assert_eq!(path, Path::new("/output/file.txt"));

        // 测试空路径
        let path = engine.build_output_path(out_dir, "", "file.txt");
        assert_eq!(path, Path::new("/output/file.txt"));

        // 测试无前导斜杠
        let path = engine.build_output_path(out_dir, "documents", "file.txt");
        assert_eq!(path, Path::new("/output/documents/file.txt"));
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip_aes() {
        let key = [0x42u8; 32];
        let nonce = [0x01u8; 12];
        let plaintext = b"Hello, World!";

        let ciphertext = encrypt_aes_gcm(plaintext, &key, &nonce).unwrap();
        let decrypted = decrypt_aes_gcm(&ciphertext, &key, &nonce).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip_chacha() {
        let key = [0x42u8; 32];
        let nonce = [0x01u8; 12];
        let plaintext = b"Hello, World!";

        let ciphertext = encrypt_chacha20_poly1305(plaintext, &key, &nonce).unwrap();
        let decrypted = decrypt_chacha20_poly1305(&ciphertext, &key, &nonce).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_creates_parent_directories() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("encrypted.dat");
        let output_path = temp_dir.path().join("nested/dir/structure/decrypted.txt");

        let key = [0x42u8; 32];
        let plaintext = b"Test content";

        // 创建加密文件
        create_encrypted_file(&input_path, plaintext, &key, EncryptionAlgorithm::Aes256Gcm).unwrap();

        // 解密到嵌套目录
        let engine = DecryptEngine::new();
        let key_info = create_test_key(&key, 1);
        engine.decrypt_file(&input_path, &output_path, &key_info).unwrap();

        // 验证目录和文件都被创建
        assert!(output_path.exists());
        let decrypted = fs::read(&output_path).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    // ========================================================================
    // 批量解密测试
    // ========================================================================

    use crate::types::{EncryptionConfig, MappingExport, MappingRecord};

    /// 创建测试用的映射记录
    fn create_test_mapping_record(
        encrypted_name: &str,
        original_path: &str,
        original_name: &str,
        key_version: u32,
    ) -> MappingRecord {
        MappingRecord {
            config_id: "test-config".to_string(),
            encrypted_name: encrypted_name.to_string(),
            original_path: original_path.to_string(),
            original_name: original_name.to_string(),
            is_directory: false,
            version: 1,
            key_version,
            file_size: 1024,
            nonce: "dGVzdG5vbmNl".to_string(),
            algorithm: "aes256gcm".to_string(),
            remote_path: None,
            status: None,
        }
    }

    /// 创建测试用的加密配置
    fn create_test_encryption_config(key_bytes: &[u8; 32], version: u32) -> EncryptionConfig {
        EncryptionConfig {
            current: create_test_key(key_bytes, version),
            history: vec![],
        }
    }

    #[test]
    fn test_decrypt_directory_single_file() {
        let temp_dir = TempDir::new().unwrap();
        let in_dir = temp_dir.path().join("input");
        let out_dir = temp_dir.path().join("output");
        fs::create_dir_all(&in_dir).unwrap();

        let key = [0x42u8; 32];
        let plaintext = b"Test file content";

        // 创建加密文件
        let encrypted_name = "test-uuid-1234.dat";
        let input_path = in_dir.join(encrypted_name);
        create_encrypted_file(&input_path, plaintext, &key, EncryptionAlgorithm::Aes256Gcm).unwrap();

        // 创建映射
        let mapping_export = MappingExport {
            version: "1.0".to_string(),
            exported_at: 1702454400000,
            records: vec![create_test_mapping_record(
                encrypted_name,
                "/documents",
                "original.txt",
                1,
            )],
        };
        let mapping = MappingLoader::from_export(mapping_export);

        // 创建密钥加载器
        let config = create_test_encryption_config(&key, 1);
        let key_loader = KeyLoader::from_config(config);

        // 批量解密
        let engine = DecryptEngine::new();
        let summary = engine.decrypt_directory(&in_dir, &out_dir, &mapping, &key_loader, false);

        // 验证结果
        assert_eq!(summary.success_count, 1);
        assert_eq!(summary.failed_count, 0);
        assert_eq!(summary.skipped_count, 0);

        // 验证输出文件
        let output_path = out_dir.join("documents/original.txt");
        assert!(output_path.exists());
        let decrypted = fs::read(&output_path).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_directory_multiple_files() {
        let temp_dir = TempDir::new().unwrap();
        let in_dir = temp_dir.path().join("input");
        let out_dir = temp_dir.path().join("output");
        fs::create_dir_all(&in_dir).unwrap();

        let key = [0x42u8; 32];

        // 创建多个加密文件
        let files = vec![
            ("file1.dat", "/docs", "report.pdf", b"Report content".to_vec()),
            ("file2.dat", "/photos", "image.jpg", b"Image data".to_vec()),
            ("file3.dat", "/", "readme.txt", b"Readme content".to_vec()),
        ];

        let mut records = Vec::new();
        for (encrypted_name, original_path, original_name, plaintext) in &files {
            let input_path = in_dir.join(encrypted_name);
            create_encrypted_file(&input_path, plaintext, &key, EncryptionAlgorithm::Aes256Gcm).unwrap();
            records.push(create_test_mapping_record(
                encrypted_name,
                original_path,
                original_name,
                1,
            ));
        }

        // 创建映射
        let mapping_export = MappingExport {
            version: "1.0".to_string(),
            exported_at: 1702454400000,
            records,
        };
        let mapping = MappingLoader::from_export(mapping_export);

        // 创建密钥加载器
        let config = create_test_encryption_config(&key, 1);
        let key_loader = KeyLoader::from_config(config);

        // 批量解密
        let engine = DecryptEngine::new();
        let summary = engine.decrypt_directory(&in_dir, &out_dir, &mapping, &key_loader, false);

        // 验证结果
        assert_eq!(summary.success_count, 3);
        assert_eq!(summary.failed_count, 0);
        assert_eq!(summary.skipped_count, 0);

        // 验证输出文件
        for (_, original_path, original_name, plaintext) in &files {
            let clean_path = original_path.trim_start_matches('/');
            let output_path = if clean_path.is_empty() {
                out_dir.join(original_name)
            } else {
                out_dir.join(clean_path).join(original_name)
            };
            assert!(output_path.exists(), "文件应该存在: {:?}", output_path);
            let decrypted = fs::read(&output_path).unwrap();
            assert_eq!(decrypted, *plaintext);
        }
    }

    #[test]
    fn test_decrypt_directory_with_subdirectories() {
        let temp_dir = TempDir::new().unwrap();
        let in_dir = temp_dir.path().join("input");
        let out_dir = temp_dir.path().join("output");
        
        // 创建输入目录结构
        let sub_dir = in_dir.join("subdir");
        fs::create_dir_all(&sub_dir).unwrap();

        let key = [0x42u8; 32];

        // 在根目录创建文件
        let root_file = "root.dat";
        create_encrypted_file(
            &in_dir.join(root_file),
            b"Root content",
            &key,
            EncryptionAlgorithm::Aes256Gcm,
        ).unwrap();

        // 在子目录创建文件
        let sub_file = "sub.dat";
        create_encrypted_file(
            &sub_dir.join(sub_file),
            b"Sub content",
            &key,
            EncryptionAlgorithm::Aes256Gcm,
        ).unwrap();

        // 创建映射
        let mapping_export = MappingExport {
            version: "1.0".to_string(),
            exported_at: 1702454400000,
            records: vec![
                create_test_mapping_record(root_file, "/", "root.txt", 1),
                create_test_mapping_record(sub_file, "/nested/path", "sub.txt", 1),
            ],
        };
        let mapping = MappingLoader::from_export(mapping_export);

        // 创建密钥加载器
        let config = create_test_encryption_config(&key, 1);
        let key_loader = KeyLoader::from_config(config);

        // 批量解密
        let engine = DecryptEngine::new();
        let summary = engine.decrypt_directory(&in_dir, &out_dir, &mapping, &key_loader, false);

        // 验证结果
        assert_eq!(summary.success_count, 2);
        assert_eq!(summary.failed_count, 0);

        // 验证输出文件
        assert!(out_dir.join("root.txt").exists());
        assert!(out_dir.join("nested/path/sub.txt").exists());
    }

    #[test]
    fn test_decrypt_directory_missing_mapping() {
        let temp_dir = TempDir::new().unwrap();
        let in_dir = temp_dir.path().join("input");
        let out_dir = temp_dir.path().join("output");
        fs::create_dir_all(&in_dir).unwrap();

        let key = [0x42u8; 32];

        // 创建加密文件
        create_encrypted_file(
            &in_dir.join("unmapped.dat"),
            b"Content",
            &key,
            EncryptionAlgorithm::Aes256Gcm,
        ).unwrap();

        // 创建空映射
        let mapping_export = MappingExport {
            version: "1.0".to_string(),
            exported_at: 1702454400000,
            records: vec![],
        };
        let mapping = MappingLoader::from_export(mapping_export);

        // 创建密钥加载器
        let config = create_test_encryption_config(&key, 1);
        let key_loader = KeyLoader::from_config(config);

        // 批量解密
        let engine = DecryptEngine::new();
        let summary = engine.decrypt_directory(&in_dir, &out_dir, &mapping, &key_loader, false);

        // 验证结果 - 文件应该被跳过
        assert_eq!(summary.success_count, 0);
        assert_eq!(summary.skipped_count, 1);
        assert_eq!(summary.failed_count, 0);
    }

    #[test]
    fn test_decrypt_directory_missing_key_version() {
        let temp_dir = TempDir::new().unwrap();
        let in_dir = temp_dir.path().join("input");
        let out_dir = temp_dir.path().join("output");
        fs::create_dir_all(&in_dir).unwrap();

        let key = [0x42u8; 32];
        let encrypted_name = "test.dat";

        // 创建加密文件
        create_encrypted_file(
            &in_dir.join(encrypted_name),
            b"Content",
            &key,
            EncryptionAlgorithm::Aes256Gcm,
        ).unwrap();

        // 创建映射（使用版本 2）
        let mapping_export = MappingExport {
            version: "1.0".to_string(),
            exported_at: 1702454400000,
            records: vec![create_test_mapping_record(
                encrypted_name,
                "/",
                "output.txt",
                2, // 版本 2
            )],
        };
        let mapping = MappingLoader::from_export(mapping_export);

        // 创建密钥加载器（只有版本 1）
        let config = create_test_encryption_config(&key, 1);
        let key_loader = KeyLoader::from_config(config);

        // 批量解密
        let engine = DecryptEngine::new();
        let summary = engine.decrypt_directory(&in_dir, &out_dir, &mapping, &key_loader, false);

        // 验证结果 - 文件应该失败
        assert_eq!(summary.success_count, 0);
        assert_eq!(summary.skipped_count, 0);
        assert_eq!(summary.failed_count, 1);
    }

    #[test]
    fn test_decrypt_directory_non_encrypted_file() {
        let temp_dir = TempDir::new().unwrap();
        let in_dir = temp_dir.path().join("input");
        let out_dir = temp_dir.path().join("output");
        fs::create_dir_all(&in_dir).unwrap();

        // 创建普通文件（非加密）
        let plain_file = in_dir.join("plain.txt");
        fs::write(&plain_file, b"Plain text content").unwrap();

        // 创建空映射
        let mapping_export = MappingExport {
            version: "1.0".to_string(),
            exported_at: 1702454400000,
            records: vec![],
        };
        let mapping = MappingLoader::from_export(mapping_export);

        // 创建密钥加载器
        let key = [0x42u8; 32];
        let config = create_test_encryption_config(&key, 1);
        let key_loader = KeyLoader::from_config(config);

        // 批量解密
        let engine = DecryptEngine::new();
        let summary = engine.decrypt_directory(&in_dir, &out_dir, &mapping, &key_loader, false);

        // 验证结果 - 非加密文件应该被跳过
        assert_eq!(summary.success_count, 0);
        assert_eq!(summary.skipped_count, 1);
        assert_eq!(summary.failed_count, 0);
    }

    #[test]
    fn test_decrypt_directory_empty() {
        let temp_dir = TempDir::new().unwrap();
        let in_dir = temp_dir.path().join("input");
        let out_dir = temp_dir.path().join("output");
        fs::create_dir_all(&in_dir).unwrap();

        // 创建空映射
        let mapping_export = MappingExport {
            version: "1.0".to_string(),
            exported_at: 1702454400000,
            records: vec![],
        };
        let mapping = MappingLoader::from_export(mapping_export);

        // 创建密钥加载器
        let key = [0x42u8; 32];
        let config = create_test_encryption_config(&key, 1);
        let key_loader = KeyLoader::from_config(config);

        // 批量解密空目录
        let engine = DecryptEngine::new();
        let summary = engine.decrypt_directory(&in_dir, &out_dir, &mapping, &key_loader, false);

        // 验证结果
        assert_eq!(summary.success_count, 0);
        assert_eq!(summary.skipped_count, 0);
        assert_eq!(summary.failed_count, 0);
        assert_eq!(summary.total_count(), 0);
    }
}


// ============================================================================
// 属性测试 (Property-Based Tests)
// ============================================================================

#[cfg(test)]
mod proptests {
    use super::*;
    use crate::types::{EncryptionAlgorithm, FILE_MAGIC};
    use proptest::prelude::*;
    use std::io::Write;
    use tempfile::TempDir;

    /// 生成随机的 32 字节密钥
    fn arb_key() -> impl Strategy<Value = [u8; 32]> {
        prop::array::uniform32(any::<u8>())
    }

    /// 生成随机的 12 字节 nonce
    fn arb_nonce() -> impl Strategy<Value = [u8; 12]> {
        prop::array::uniform12(any::<u8>())
    }

    /// 生成随机的明文数据（1 字节到 64KB）
    #[allow(dead_code)]
    fn arb_plaintext() -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(any::<u8>(), 1..65536)
    }

    /// 生成随机的小明文数据（1 字节到 1KB，用于快速测试）
    fn arb_small_plaintext() -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(any::<u8>(), 1..1024)
    }

    /// 创建测试用的密钥信息
    fn create_test_key_info(key_bytes: &[u8; 32], version: u32) -> EncryptionKeyInfo {
        EncryptionKeyInfo {
            master_key: base64::engine::general_purpose::STANDARD.encode(key_bytes),
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            key_version: version,
            created_at: 1702454400000,
            last_used_at: None,
            deprecated_at: None,
        }
    }

    /// 创建加密文件（用于属性测试）
    fn create_encrypted_file_for_test(
        path: &std::path::Path,
        plaintext: &[u8],
        key: &[u8; 32],
        nonce: &[u8; 12],
        algorithm: EncryptionAlgorithm,
    ) -> std::io::Result<()> {
        let mut file = std::fs::File::create(path)?;

        // 写入文件头
        file.write_all(&FILE_MAGIC)?;
        file.write_all(&[algorithm.to_byte()])?;
        file.write_all(nonce)?;
        file.write_all(&(plaintext.len() as u64).to_le_bytes())?;
        file.write_all(&1u32.to_le_bytes())?; // 单个分块

        // 加密数据
        let ciphertext = match algorithm {
            EncryptionAlgorithm::Aes256Gcm => {
                encrypt_aes_gcm(plaintext, key, nonce).unwrap()
            }
            EncryptionAlgorithm::ChaCha20Poly1305 => {
                encrypt_chacha20_poly1305(plaintext, key, nonce).unwrap()
            }
        };

        // 写入分块
        file.write_all(nonce)?;
        file.write_all(&(ciphertext.len() as u32).to_le_bytes())?;
        file.write_all(&ciphertext)?;

        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        // ====================================================================
        // Property 2: 加密解密往返一致性
        //
        // *For any* 有效的原始文件，使用相同密钥进行加密后再解密，
        // 应该得到与原始文件完全相同的内容。
        //
        // **Validates: Requirements 2.4, 3.3, 4.1, 4.2**
        // ====================================================================

        /// Property 2.1: AES-256-GCM 加密解密往返一致性
        ///
        /// 对于任意明文和密钥，使用 AES-256-GCM 加密后解密应得到原始明文。
        ///
        /// **Validates: Requirements 2.4, 3.3, 4.1**
        #[test]
        fn prop_aes_gcm_roundtrip(
            plaintext in arb_small_plaintext(),
            key in arb_key(),
            nonce in arb_nonce(),
        ) {
            // 加密
            let ciphertext = encrypt_aes_gcm(&plaintext, &key, &nonce)
                .expect("AES-GCM 加密应该成功");

            // 解密
            let decrypted = decrypt_aes_gcm(&ciphertext, &key, &nonce)
                .expect("AES-GCM 解密应该成功");

            // 验证往返一致性
            prop_assert_eq!(
                decrypted,
                plaintext,
                "AES-GCM 解密后的数据应该与原始明文完全相同"
            );
        }

        /// Property 2.2: ChaCha20-Poly1305 加密解密往返一致性
        ///
        /// 对于任意明文和密钥，使用 ChaCha20-Poly1305 加密后解密应得到原始明文。
        ///
        /// **Validates: Requirements 2.4, 3.3, 4.2**
        #[test]
        fn prop_chacha20_poly1305_roundtrip(
            plaintext in arb_small_plaintext(),
            key in arb_key(),
            nonce in arb_nonce(),
        ) {
            // 加密
            let ciphertext = encrypt_chacha20_poly1305(&plaintext, &key, &nonce)
                .expect("ChaCha20-Poly1305 加密应该成功");

            // 解密
            let decrypted = decrypt_chacha20_poly1305(&ciphertext, &key, &nonce)
                .expect("ChaCha20-Poly1305 解密应该成功");

            // 验证往返一致性
            prop_assert_eq!(
                decrypted,
                plaintext,
                "ChaCha20-Poly1305 解密后的数据应该与原始明文完全相同"
            );
        }

        /// Property 2.3: 文件级 AES-256-GCM 加密解密往返一致性
        ///
        /// 对于任意文件内容，通过 DecryptEngine 解密加密文件应得到原始内容。
        ///
        /// **Validates: Requirements 2.4, 3.3, 4.1**
        #[test]
        fn prop_file_aes_gcm_roundtrip(
            plaintext in arb_small_plaintext(),
            key in arb_key(),
            nonce in arb_nonce(),
        ) {
            let temp_dir = TempDir::new().unwrap();
            let input_path = temp_dir.path().join("encrypted.dat");
            let output_path = temp_dir.path().join("decrypted.bin");

            // 创建加密文件
            create_encrypted_file_for_test(
                &input_path,
                &plaintext,
                &key,
                &nonce,
                EncryptionAlgorithm::Aes256Gcm,
            ).expect("创建加密文件应该成功");

            // 解密
            let engine = DecryptEngine::new();
            let key_info = create_test_key_info(&key, 1);
            engine.decrypt_file(&input_path, &output_path, &key_info)
                .expect("文件解密应该成功");

            // 验证
            let decrypted = std::fs::read(&output_path).expect("读取解密文件应该成功");
            prop_assert_eq!(
                decrypted,
                plaintext,
                "文件解密后的内容应该与原始明文完全相同"
            );
        }

        /// Property 2.4: 文件级 ChaCha20-Poly1305 加密解密往返一致性
        ///
        /// 对于任意文件内容，通过 DecryptEngine 解密加密文件应得到原始内容。
        ///
        /// **Validates: Requirements 2.4, 3.3, 4.2**
        #[test]
        fn prop_file_chacha20_roundtrip(
            plaintext in arb_small_plaintext(),
            key in arb_key(),
            nonce in arb_nonce(),
        ) {
            let temp_dir = TempDir::new().unwrap();
            let input_path = temp_dir.path().join("encrypted.dat");
            let output_path = temp_dir.path().join("decrypted.bin");

            // 创建加密文件
            create_encrypted_file_for_test(
                &input_path,
                &plaintext,
                &key,
                &nonce,
                EncryptionAlgorithm::ChaCha20Poly1305,
            ).expect("创建加密文件应该成功");

            // 解密
            let engine = DecryptEngine::new();
            let key_info = create_test_key_info(&key, 1);
            engine.decrypt_file(&input_path, &output_path, &key_info)
                .expect("文件解密应该成功");

            // 验证
            let decrypted = std::fs::read(&output_path).expect("读取解密文件应该成功");
            prop_assert_eq!(
                decrypted,
                plaintext,
                "文件解密后的内容应该与原始明文完全相同"
            );
        }

        /// Property 2.5: 错误密钥应该导致解密失败
        ///
        /// 使用错误的密钥解密应该失败，不应产生任何有效输出。
        ///
        /// **Validates: Requirements 3.3, 4.1, 4.2**
        #[test]
        fn prop_wrong_key_fails(
            plaintext in arb_small_plaintext(),
            correct_key in arb_key(),
            wrong_key in arb_key(),
            nonce in arb_nonce(),
        ) {
            // 确保两个密钥不同
            prop_assume!(correct_key != wrong_key);

            // 使用正确密钥加密
            let ciphertext = encrypt_aes_gcm(&plaintext, &correct_key, &nonce)
                .expect("加密应该成功");

            // 使用错误密钥解密应该失败
            let result = decrypt_aes_gcm(&ciphertext, &wrong_key, &nonce);
            prop_assert!(
                result.is_err(),
                "使用错误密钥解密应该失败"
            );
        }

        /// Property 2.6: 密文长度应该大于明文长度（包含认证标签）
        ///
        /// 加密后的密文应该比明文长 16 字节（认证标签大小）。
        ///
        /// **Validates: Requirements 4.1, 4.2**
        #[test]
        fn prop_ciphertext_length(
            plaintext in arb_small_plaintext(),
            key in arb_key(),
            nonce in arb_nonce(),
        ) {
            // AES-GCM
            let aes_ciphertext = encrypt_aes_gcm(&plaintext, &key, &nonce)
                .expect("AES-GCM 加密应该成功");
            prop_assert_eq!(
                aes_ciphertext.len(),
                plaintext.len() + 16,
                "AES-GCM 密文长度应该是明文长度 + 16 字节认证标签"
            );

            // ChaCha20-Poly1305
            let chacha_ciphertext = encrypt_chacha20_poly1305(&plaintext, &key, &nonce)
                .expect("ChaCha20-Poly1305 加密应该成功");
            prop_assert_eq!(
                chacha_ciphertext.len(),
                plaintext.len() + 16,
                "ChaCha20-Poly1305 密文长度应该是明文长度 + 16 字节认证标签"
            );
        }

        /// Property 2.7: 相同输入应该产生相同输出（确定性）
        ///
        /// 使用相同的密钥、nonce 和明文，加密结果应该相同。
        ///
        /// **Validates: Requirements 4.1, 4.2**
        #[test]
        fn prop_encryption_deterministic(
            plaintext in arb_small_plaintext(),
            key in arb_key(),
            nonce in arb_nonce(),
        ) {
            // AES-GCM
            let aes_ct1 = encrypt_aes_gcm(&plaintext, &key, &nonce).unwrap();
            let aes_ct2 = encrypt_aes_gcm(&plaintext, &key, &nonce).unwrap();
            prop_assert_eq!(aes_ct1, aes_ct2, "AES-GCM 加密应该是确定性的");

            // ChaCha20-Poly1305
            let chacha_ct1 = encrypt_chacha20_poly1305(&plaintext, &key, &nonce).unwrap();
            let chacha_ct2 = encrypt_chacha20_poly1305(&plaintext, &key, &nonce).unwrap();
            prop_assert_eq!(chacha_ct1, chacha_ct2, "ChaCha20-Poly1305 加密应该是确定性的");
        }

        /// Property 2.8: 不同 nonce 应该产生不同密文
        ///
        /// 使用不同的 nonce 加密相同明文应该产生不同的密文。
        ///
        /// **Validates: Requirements 4.1, 4.2**
        #[test]
        fn prop_different_nonce_different_ciphertext(
            plaintext in arb_small_plaintext(),
            key in arb_key(),
            nonce1 in arb_nonce(),
            nonce2 in arb_nonce(),
        ) {
            prop_assume!(nonce1 != nonce2);

            // AES-GCM
            let aes_ct1 = encrypt_aes_gcm(&plaintext, &key, &nonce1).unwrap();
            let aes_ct2 = encrypt_aes_gcm(&plaintext, &key, &nonce2).unwrap();
            prop_assert_ne!(
                aes_ct1,
                aes_ct2,
                "不同 nonce 的 AES-GCM 密文应该不同"
            );

            // ChaCha20-Poly1305
            let chacha_ct1 = encrypt_chacha20_poly1305(&plaintext, &key, &nonce1).unwrap();
            let chacha_ct2 = encrypt_chacha20_poly1305(&plaintext, &key, &nonce2).unwrap();
            prop_assert_ne!(
                chacha_ct1,
                chacha_ct2,
                "不同 nonce 的 ChaCha20-Poly1305 密文应该不同"
            );
        }

        // ====================================================================
        // Property 3: 目录结构恢复正确性
        //
        // *For any* 包含映射记录的加密文件集合，批量解密后输出目录结构应该与
        // 映射中记录的 original_path/original_name 完全一致，包括所有嵌套的子目录。
        //
        // **Validates: Requirements 2.4, 2.5, 2.6**
        // ====================================================================

        /// Property 3.1: 输出路径应该正确构建
        ///
        /// 对于任意 original_path 和 original_name，build_output_path 应该
        /// 正确构建输出路径。
        ///
        /// **Validates: Requirements 2.4, 2.5**
        #[test]
        fn prop_output_path_construction(
            // 使用更合理的路径格式，避免生成 UNC 路径（如 //server）
            out_dir in "[a-zA-Z][a-zA-Z0-9_]{0,10}(/[a-zA-Z0-9_]{1,5}){0,3}",
            original_path in prop_oneof![
                Just("/".to_string()),
                Just("".to_string()),
                "/[a-zA-Z0-9_]{1,10}(/[a-zA-Z0-9_]{1,10}){0,3}",
            ],
            original_name in "[a-zA-Z0-9_]{1,10}\\.[a-z]{2,4}",
        ) {
            let engine = DecryptEngine::new();
            let out_dir_path = std::path::Path::new(&out_dir);
            let output_path = engine.build_output_path(out_dir_path, &original_path, &original_name);

            // 输出路径应该以 out_dir 开头
            prop_assert!(
                output_path.starts_with(out_dir_path),
                "输出路径应该以 out_dir 开头: {:?}",
                output_path
            );

            // 输出路径应该以 original_name 结尾
            prop_assert_eq!(
                output_path.file_name().and_then(|n| n.to_str()),
                Some(original_name.as_str()),
                "输出路径应该以 original_name 结尾"
            );

            // 如果 original_path 非空且非根，输出路径应该包含它
            let clean_path = original_path.trim_start_matches('/').trim_start_matches('\\');
            if !clean_path.is_empty() {
                let expected_parent = out_dir_path.join(clean_path);
                prop_assert_eq!(
                    output_path.parent(),
                    Some(expected_parent.as_path()),
                    "输出路径的父目录应该正确"
                );
            }
        }

        /// Property 3.2: 批量解密应该恢复正确的目录结构
        ///
        /// 对于任意文件集合，批量解密后的输出目录结构应该与映射记录一致。
        ///
        /// **Validates: Requirements 2.4, 2.5, 2.6**
        #[test]
        fn prop_directory_structure_restoration(
            // 生成 1-5 个文件的配置
            file_count in 1usize..=5,
            key in arb_key(),
            nonce in arb_nonce(),
        ) {
            use crate::types::{EncryptionConfig, MappingExport, MappingRecord};

            let temp_dir = TempDir::new().unwrap();
            let in_dir = temp_dir.path().join("input");
            let out_dir = temp_dir.path().join("output");
            std::fs::create_dir_all(&in_dir).unwrap();

            // 生成文件配置
            let paths = vec![
                ("/", "root.txt"),
                ("/docs", "report.pdf"),
                ("/docs/work", "project.doc"),
                ("/photos/2024", "image.jpg"),
                ("/backup/old/archive", "data.zip"),
            ];

            let mut records = Vec::new();
            let mut expected_outputs = Vec::new();

            for i in 0..file_count {
                let (original_path, original_name) = paths[i % paths.len()];
                let encrypted_name = format!("file-{}.dat", i);
                let plaintext = format!("Content of file {}", i).into_bytes();

                // 创建加密文件
                let input_path = in_dir.join(&encrypted_name);
                create_encrypted_file_for_test(
                    &input_path,
                    &plaintext,
                    &key,
                    &nonce,
                    EncryptionAlgorithm::Aes256Gcm,
                ).unwrap();

                // 创建映射记录
                records.push(MappingRecord {
                    config_id: "test".to_string(),
                    encrypted_name: encrypted_name.clone(),
                    original_path: original_path.to_string(),
                    original_name: original_name.to_string(),
                    is_directory: false,
                    version: 1,
                    key_version: 1,
                    file_size: plaintext.len() as u64,
                    nonce: base64::engine::general_purpose::STANDARD.encode(&nonce),
                    algorithm: "aes256gcm".to_string(),
                    remote_path: None,
                    status: None,
                });

                // 计算期望的输出路径
                let clean_path = original_path.trim_start_matches('/');
                let expected_path = if clean_path.is_empty() {
                    out_dir.join(original_name)
                } else {
                    out_dir.join(clean_path).join(original_name)
                };
                expected_outputs.push((expected_path, plaintext));
            }

            // 创建映射加载器
            let mapping_export = MappingExport {
                version: "1.0".to_string(),
                exported_at: 1702454400000,
                records,
            };
            let mapping = MappingLoader::from_export(mapping_export);

            // 创建密钥加载器
            let config = EncryptionConfig {
                current: create_test_key_info(&key, 1),
                history: vec![],
            };
            let key_loader = KeyLoader::from_config(config);

            // 执行批量解密
            let engine = DecryptEngine::new();
            let summary = engine.decrypt_directory(&in_dir, &out_dir, &mapping, &key_loader, false);

            // 验证所有文件都成功解密
            prop_assert_eq!(
                summary.success_count,
                file_count,
                "所有文件应该成功解密"
            );
            prop_assert_eq!(
                summary.failed_count,
                0,
                "不应该有失败的文件"
            );

            // 验证输出文件存在且内容正确
            for (expected_path, expected_content) in &expected_outputs {
                prop_assert!(
                    expected_path.exists(),
                    "输出文件应该存在: {:?}",
                    expected_path
                );

                let actual_content = std::fs::read(expected_path).unwrap();
                prop_assert_eq!(
                    &actual_content,
                    expected_content,
                    "文件内容应该正确: {:?}",
                    expected_path
                );
            }
        }

        /// Property 3.3: 嵌套目录应该被自动创建
        ///
        /// 对于任意深度的目录路径，批量解密应该自动创建所有必要的父目录。
        ///
        /// **Validates: Requirements 2.6**
        #[test]
        fn prop_nested_directories_created(
            depth in 1usize..=5,
            key in arb_key(),
            nonce in arb_nonce(),
        ) {
            use crate::types::{EncryptionConfig, MappingExport, MappingRecord};

            let temp_dir = TempDir::new().unwrap();
            let in_dir = temp_dir.path().join("input");
            let out_dir = temp_dir.path().join("output");
            std::fs::create_dir_all(&in_dir).unwrap();

            // 构建嵌套路径
            let mut path_parts = Vec::new();
            for i in 0..depth {
                path_parts.push(format!("level{}", i));
            }
            let original_path = format!("/{}", path_parts.join("/"));
            let original_name = "deep_file.txt";
            let encrypted_name = "deep.dat";
            let plaintext = b"Deep content";

            // 创建加密文件
            let input_path = in_dir.join(encrypted_name);
            create_encrypted_file_for_test(
                &input_path,
                plaintext,
                &key,
                &nonce,
                EncryptionAlgorithm::Aes256Gcm,
            ).unwrap();

            // 创建映射
            let mapping_export = MappingExport {
                version: "1.0".to_string(),
                exported_at: 1702454400000,
                records: vec![MappingRecord {
                    config_id: "test".to_string(),
                    encrypted_name: encrypted_name.to_string(),
                    original_path: original_path.clone(),
                    original_name: original_name.to_string(),
                    is_directory: false,
                    version: 1,
                    key_version: 1,
                    file_size: plaintext.len() as u64,
                    nonce: base64::engine::general_purpose::STANDARD.encode(&nonce),
                    algorithm: "aes256gcm".to_string(),
                    remote_path: None,
                    status: None,
                }],
            };
            let mapping = MappingLoader::from_export(mapping_export);

            // 创建密钥加载器
            let config = EncryptionConfig {
                current: create_test_key_info(&key, 1),
                history: vec![],
            };
            let key_loader = KeyLoader::from_config(config);

            // 执行批量解密
            let engine = DecryptEngine::new();
            let summary = engine.decrypt_directory(&in_dir, &out_dir, &mapping, &key_loader, false);

            // 验证成功
            prop_assert_eq!(summary.success_count, 1, "文件应该成功解密");

            // 验证所有目录层级都被创建
            let mut current_path = out_dir.clone();
            for part in &path_parts {
                current_path = current_path.join(part);
                prop_assert!(
                    current_path.exists() && current_path.is_dir(),
                    "目录应该存在: {:?}",
                    current_path
                );
            }

            // 验证文件存在
            let file_path = current_path.join(original_name);
            prop_assert!(file_path.exists(), "文件应该存在: {:?}", file_path);
        }

        // ====================================================================
        // Property 9: 单文件模式独立性
        //
        // *For any* 加密文件，单文件解密模式应该能够在没有 mapping.json 的情况下
        // 正常工作，并且能够通过遍历所有密钥找到正确的密钥进行解密。
        //
        // **Validates: Requirements 3.1, 3.2, 5.3**
        // ====================================================================

        /// Property 9.1: 单文件解密不依赖 mapping.json
        ///
        /// 单文件解密模式应该能够在没有映射文件的情况下正常工作，
        /// 只需要密钥文件和输入/输出路径。
        ///
        /// **Validates: Requirements 3.1, 3.2**
        #[test]
        fn prop_single_file_mode_no_mapping_required(
            key in arb_key(),
            nonce in arb_nonce(),
            plaintext in prop::collection::vec(any::<u8>(), 1..1024),
            algo in prop_oneof![
                Just(EncryptionAlgorithm::Aes256Gcm),
                Just(EncryptionAlgorithm::ChaCha20Poly1305),
            ],
        ) {
            let temp_dir = TempDir::new().unwrap();
            let input_path = temp_dir.path().join("encrypted.dat");
            let output_path = temp_dir.path().join("decrypted.bin");

            // 创建加密文件
            create_encrypted_file_for_test(
                &input_path,
                &plaintext,
                &key,
                &nonce,
                algo,
            ).unwrap();

            // 创建密钥信息（不需要映射）
            let key_info = create_test_key_info(&key, 1);

            // 直接解密单个文件，不使用映射
            let engine = DecryptEngine::new();
            let result = engine.decrypt_file(&input_path, &output_path, &key_info);

            // 验证解密成功
            prop_assert!(
                result.is_ok(),
                "单文件解密应该成功（无需映射）: {:?}",
                result
            );

            // 验证内容正确
            let decrypted = std::fs::read(&output_path).unwrap();
            prop_assert_eq!(
                decrypted,
                plaintext,
                "解密内容应该与原始内容一致"
            );
        }

        /// Property 9.2: 密钥遍历逻辑正确性
        ///
        /// 当不指定 key_version 时，单文件解密应该遍历所有可用密钥，
        /// 直到找到能够成功解密的密钥。
        ///
        /// **Validates: Requirements 5.3**
        #[test]
        fn prop_key_traversal_finds_correct_key(
            correct_key in arb_key(),
            wrong_key1 in arb_key(),
            wrong_key2 in arb_key(),
            nonce in arb_nonce(),
            plaintext in prop::collection::vec(any::<u8>(), 1..512),
        ) {
            // 确保密钥不同
            prop_assume!(correct_key != wrong_key1);
            prop_assume!(correct_key != wrong_key2);
            prop_assume!(wrong_key1 != wrong_key2);

            let temp_dir = TempDir::new().unwrap();
            let input_path = temp_dir.path().join("encrypted.dat");
            let output_path = temp_dir.path().join("decrypted.bin");

            // 使用 correct_key 创建加密文件
            create_encrypted_file_for_test(
                &input_path,
                &plaintext,
                &correct_key,
                &nonce,
                EncryptionAlgorithm::Aes256Gcm,
            ).unwrap();

            // 创建多个密钥（正确的密钥在中间位置）
            let key_info1 = create_test_key_info(&wrong_key1, 1);
            let key_info2 = create_test_key_info(&correct_key, 2);
            let key_info3 = create_test_key_info(&wrong_key2, 3);

            let keys: Vec<&EncryptionKeyInfo> = vec![&key_info1, &key_info2, &key_info3];

            // 使用密钥遍历解密
            let engine = DecryptEngine::new();
            let result = engine.decrypt_file_with_any_key(&input_path, &output_path, &keys);

            // 验证解密成功
            prop_assert!(
                result.is_ok(),
                "密钥遍历应该找到正确的密钥并成功解密: {:?}",
                result
            );

            // 验证内容正确
            let decrypted = std::fs::read(&output_path).unwrap();
            prop_assert_eq!(
                decrypted,
                plaintext,
                "解密内容应该与原始内容一致"
            );
        }

        /// Property 9.3: 密钥遍历在所有密钥都错误时失败
        ///
        /// 当所有可用密钥都无法解密文件时，应该返回解密失败错误。
        ///
        /// **Validates: Requirements 5.3**
        #[test]
        fn prop_key_traversal_fails_when_no_key_matches(
            correct_key in arb_key(),
            wrong_key1 in arb_key(),
            wrong_key2 in arb_key(),
            nonce in arb_nonce(),
            plaintext in prop::collection::vec(any::<u8>(), 1..512),
        ) {
            // 确保所有密钥都不同
            prop_assume!(correct_key != wrong_key1);
            prop_assume!(correct_key != wrong_key2);
            prop_assume!(wrong_key1 != wrong_key2);

            let temp_dir = TempDir::new().unwrap();
            let input_path = temp_dir.path().join("encrypted.dat");
            let output_path = temp_dir.path().join("decrypted.bin");

            // 使用 correct_key 创建加密文件
            create_encrypted_file_for_test(
                &input_path,
                &plaintext,
                &correct_key,
                &nonce,
                EncryptionAlgorithm::Aes256Gcm,
            ).unwrap();

            // 只提供错误的密钥
            let key_info1 = create_test_key_info(&wrong_key1, 1);
            let key_info2 = create_test_key_info(&wrong_key2, 2);

            let keys: Vec<&EncryptionKeyInfo> = vec![&key_info1, &key_info2];

            // 使用密钥遍历解密
            let engine = DecryptEngine::new();
            let result = engine.decrypt_file_with_any_key(&input_path, &output_path, &keys);

            // 验证解密失败
            prop_assert!(
                result.is_err(),
                "所有密钥都错误时应该返回错误"
            );

            // 验证错误类型
            match result {
                Err(DecryptError::DecryptionFailed(_)) => {
                    // 正确的错误类型
                }
                Err(e) => {
                    prop_assert!(
                        false,
                        "应该返回 DecryptionFailed 错误，实际返回: {:?}",
                        e
                    );
                }
                Ok(_) => {
                    prop_assert!(false, "不应该成功");
                }
            }
        }

        /// Property 9.4: 空密钥列表返回 KeyMismatch 错误
        ///
        /// 当没有可用密钥时，应该返回 KeyMismatch 错误。
        ///
        /// **Validates: Requirements 5.3**
        #[test]
        fn prop_empty_keys_returns_key_mismatch(
            key in arb_key(),
            nonce in arb_nonce(),
            plaintext in prop::collection::vec(any::<u8>(), 1..512),
        ) {
            let temp_dir = TempDir::new().unwrap();
            let input_path = temp_dir.path().join("encrypted.dat");
            let output_path = temp_dir.path().join("decrypted.bin");

            // 创建加密文件
            create_encrypted_file_for_test(
                &input_path,
                &plaintext,
                &key,
                &nonce,
                EncryptionAlgorithm::Aes256Gcm,
            ).unwrap();

            // 空密钥列表
            let keys: Vec<&EncryptionKeyInfo> = vec![];

            // 使用空密钥列表解密
            let engine = DecryptEngine::new();
            let result = engine.decrypt_file_with_any_key(&input_path, &output_path, &keys);

            // 验证返回 KeyMismatch 错误
            prop_assert!(result.is_err(), "空密钥列表应该返回错误");
            match result {
                Err(DecryptError::KeyMismatch(0)) => {
                    // 正确的错误类型
                }
                Err(e) => {
                    prop_assert!(
                        false,
                        "应该返回 KeyMismatch(0) 错误，实际返回: {:?}",
                        e
                    );
                }
                Ok(_) => {
                    prop_assert!(false, "不应该成功");
                }
            }
        }

        /// Property 9.5: 单文件模式支持两种加密算法
        ///
        /// 单文件解密应该能够正确处理 AES-256-GCM 和 ChaCha20-Poly1305 两种算法。
        ///
        /// **Validates: Requirements 3.1, 3.2**
        #[test]
        fn prop_single_file_supports_both_algorithms(
            key in arb_key(),
            nonce in arb_nonce(),
            plaintext in prop::collection::vec(any::<u8>(), 1..512),
        ) {
            let temp_dir = TempDir::new().unwrap();

            for algo in [EncryptionAlgorithm::Aes256Gcm, EncryptionAlgorithm::ChaCha20Poly1305] {
                let input_path = temp_dir.path().join(format!("encrypted_{:?}.dat", algo));
                let output_path = temp_dir.path().join(format!("decrypted_{:?}.bin", algo));

                // 创建加密文件
                create_encrypted_file_for_test(
                    &input_path,
                    &plaintext,
                    &key,
                    &nonce,
                    algo,
                ).unwrap();

                // 创建密钥信息
                let key_info = create_test_key_info(&key, 1);

                // 解密
                let engine = DecryptEngine::new();
                let result = engine.decrypt_file(&input_path, &output_path, &key_info);

                // 验证成功
                prop_assert!(
                    result.is_ok(),
                    "算法 {:?} 解密应该成功: {:?}",
                    algo,
                    result
                );

                // 验证内容
                let decrypted = std::fs::read(&output_path).unwrap();
                prop_assert_eq!(
                    decrypted,
                    plaintext.clone(),
                    "算法 {:?} 解密内容应该正确",
                    algo
                );
            }
        }
    }
}
