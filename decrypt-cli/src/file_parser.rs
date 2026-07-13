//! 文件头解析器
//!
//! 解析加密文件的文件头和分块数据。
//!
//! # 文件格式
//!
//! ## 文件头（31 字节）
//! ```text
//! +--------+------+---------------+---------------+--------------+
//! | magic  | algo | master_nonce  | original_size | total_chunks |
//! | 6 bytes| 1 B  | 12 bytes      | 8 bytes (LE)  | 4 bytes (LE) |
//! +--------+------+---------------+---------------+--------------+
//! ```
//!
//! ## 每个分块
//! ```text
//! +--------------+----------------+------------------------+
//! | chunk_nonce  | ciphertext_len | ciphertext             |
//! | 12 bytes     | 4 bytes (LE)   | ciphertext_len bytes   |
//! +--------------+----------------+------------------------+
//! ```

use std::io::{BufReader, Read};
use std::path::Path;

use crate::types::{DecryptError, EncryptionAlgorithm, FileHeader, FILE_HEADER_SIZE, FILE_MAGIC};

/// 分块数据
#[derive(Debug, Clone)]
pub struct ChunkData {
    /// 块 Nonce（12 字节）
    pub nonce: [u8; 12],
    /// 密文数据
    pub ciphertext: Vec<u8>,
}

/// 文件头解析器
pub struct FileHeaderParser;

impl FileHeaderParser {
    /// 从文件路径解析文件头
    #[allow(dead_code)]
    pub fn parse_from_path(path: &Path) -> Result<FileHeader, DecryptError> {
        let file = std::fs::File::open(path)?;
        let mut reader = BufReader::new(file);
        Self::parse(&mut reader)
    }

    /// 从 Reader 解析文件头
    pub fn parse<R: Read>(reader: &mut R) -> Result<FileHeader, DecryptError> {
        // 读取完整的文件头（31 字节）
        let mut header_bytes = [0u8; FILE_HEADER_SIZE];
        reader.read_exact(&mut header_bytes).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                DecryptError::InvalidFormat("文件太小，无法读取完整的文件头".to_string())
            } else {
                DecryptError::IoError(e)
            }
        })?;

        // 解析 magic（6 字节）
        let mut magic = [0u8; 6];
        magic.copy_from_slice(&header_bytes[0..6]);

        // 验证 magic
        if !Self::validate_magic(&magic) {
            return Err(DecryptError::InvalidFormat(format!(
                "无效的文件魔数: 期望 {:02X?}, 实际 {:02X?}",
                FILE_MAGIC, magic
            )));
        }

        // 解析算法标识（1 字节）
        let algo_byte = header_bytes[6];
        let algorithm = EncryptionAlgorithm::from_byte(algo_byte).ok_or_else(|| {
            DecryptError::InvalidFormat(format!("无效的算法标识: {}", algo_byte))
        })?;

        // 解析 master_nonce（12 字节）
        let mut master_nonce = [0u8; 12];
        master_nonce.copy_from_slice(&header_bytes[7..19]);

        // 解析 original_size（8 字节，小端序）
        let mut size_bytes = [0u8; 8];
        size_bytes.copy_from_slice(&header_bytes[19..27]);
        let original_size = u64::from_le_bytes(size_bytes);

        // 解析 total_chunks（4 字节，小端序）
        let mut chunks_bytes = [0u8; 4];
        chunks_bytes.copy_from_slice(&header_bytes[27..31]);
        let total_chunks = u32::from_le_bytes(chunks_bytes);

        Ok(FileHeader {
            magic,
            algorithm,
            master_nonce,
            original_size,
            total_chunks,
        })
    }

    /// 验证 magic 字节是否正确
    pub fn validate_magic(magic: &[u8; 6]) -> bool {
        magic == &FILE_MAGIC
    }

    /// 检查文件是否为有效的加密文件（仅检查 magic）
    pub fn is_encrypted_file(path: &Path) -> Result<bool, DecryptError> {
        let file = std::fs::File::open(path)?;
        let mut reader = BufReader::new(file);

        let mut magic = [0u8; 6];
        match reader.read_exact(&mut magic) {
            Ok(_) => Ok(Self::validate_magic(&magic)),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
            Err(e) => Err(DecryptError::IoError(e)),
        }
    }
}

/// 分块读取器
///
/// 用于流式读取加密文件的分块数据
pub struct ChunkReader<R: Read> {
    reader: R,
    total_chunks: u32,
    current_chunk: u32,
}

impl<R: Read> ChunkReader<R> {
    /// 创建新的分块读取器
    ///
    /// 注意：调用此方法前，应该已经读取并解析了文件头
    pub fn new(reader: R, total_chunks: u32) -> Self {
        Self {
            reader,
            total_chunks,
            current_chunk: 0,
        }
    }

    /// 获取总分块数
    #[allow(dead_code)]
    pub fn total_chunks(&self) -> u32 {
        self.total_chunks
    }

    /// 获取当前分块索引
    #[allow(dead_code)]
    pub fn current_chunk(&self) -> u32 {
        self.current_chunk
    }

    /// 是否还有更多分块
    pub fn has_more(&self) -> bool {
        self.current_chunk < self.total_chunks
    }

    /// 读取下一个分块
    pub fn read_next_chunk(&mut self) -> Result<Option<ChunkData>, DecryptError> {
        if !self.has_more() {
            return Ok(None);
        }

        // 读取块 Nonce（12 字节）
        let mut nonce = [0u8; 12];
        self.reader.read_exact(&mut nonce).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                DecryptError::CorruptedFile(format!(
                    "分块 {} 的 Nonce 数据不完整",
                    self.current_chunk
                ))
            } else {
                DecryptError::IoError(e)
            }
        })?;

        // 读取密文长度（4 字节，小端序）
        let mut len_bytes = [0u8; 4];
        self.reader.read_exact(&mut len_bytes).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                DecryptError::CorruptedFile(format!(
                    "分块 {} 的长度数据不完整",
                    self.current_chunk
                ))
            } else {
                DecryptError::IoError(e)
            }
        })?;
        let ciphertext_len = u32::from_le_bytes(len_bytes) as usize;

        // 验证密文长度合理性（防止恶意文件导致内存耗尽）
        // 最大分块大小为 16MB + 16 字节认证标签
        const MAX_CHUNK_SIZE: usize = 16 * 1024 * 1024 + 16;
        if ciphertext_len > MAX_CHUNK_SIZE {
            return Err(DecryptError::CorruptedFile(format!(
                "分块 {} 的密文长度异常: {} 字节（最大允许 {} 字节）",
                self.current_chunk, ciphertext_len, MAX_CHUNK_SIZE
            )));
        }

        // 读取密文
        let mut ciphertext = vec![0u8; ciphertext_len];
        self.reader.read_exact(&mut ciphertext).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                DecryptError::CorruptedFile(format!(
                    "分块 {} 的密文数据不完整: 期望 {} 字节",
                    self.current_chunk, ciphertext_len
                ))
            } else {
                DecryptError::IoError(e)
            }
        })?;

        self.current_chunk += 1;

        Ok(Some(ChunkData { nonce, ciphertext }))
    }
}

impl<R: Read> Iterator for ChunkReader<R> {
    type Item = Result<ChunkData, DecryptError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.read_next_chunk() {
            Ok(Some(chunk)) => Some(Ok(chunk)),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

/// 派生块 Nonce
///
/// 通过将 master_nonce 的最后 4 字节与 chunk_index 进行 XOR 运算来派生块 Nonce
#[allow(dead_code)]
pub fn derive_chunk_nonce(master_nonce: &[u8; 12], chunk_index: u32) -> [u8; 12] {
    let mut chunk_nonce = *master_nonce;
    let index_bytes = chunk_index.to_le_bytes();
    // XOR 最后 4 字节
    for i in 0..4 {
        chunk_nonce[8 + i] ^= index_bytes[i];
    }
    chunk_nonce
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// 创建有效的文件头字节
    fn create_valid_header(
        algorithm: EncryptionAlgorithm,
        original_size: u64,
        total_chunks: u32,
    ) -> Vec<u8> {
        let mut header = Vec::with_capacity(FILE_HEADER_SIZE);
        header.extend_from_slice(&FILE_MAGIC);
        header.push(algorithm.to_byte());
        header.extend_from_slice(&[0u8; 12]); // master_nonce
        header.extend_from_slice(&original_size.to_le_bytes());
        header.extend_from_slice(&total_chunks.to_le_bytes());
        header
    }

    #[test]
    fn test_parse_valid_header_aes() {
        let header_bytes = create_valid_header(EncryptionAlgorithm::Aes256Gcm, 1024, 1);
        let mut cursor = Cursor::new(header_bytes);

        let header = FileHeaderParser::parse(&mut cursor).unwrap();

        assert!(header.validate_magic());
        assert_eq!(header.algorithm, EncryptionAlgorithm::Aes256Gcm);
        assert_eq!(header.original_size, 1024);
        assert_eq!(header.total_chunks, 1);
    }

    #[test]
    fn test_parse_valid_header_chacha() {
        let header_bytes = create_valid_header(EncryptionAlgorithm::ChaCha20Poly1305, 2048, 2);
        let mut cursor = Cursor::new(header_bytes);

        let header = FileHeaderParser::parse(&mut cursor).unwrap();

        assert!(header.validate_magic());
        assert_eq!(header.algorithm, EncryptionAlgorithm::ChaCha20Poly1305);
        assert_eq!(header.original_size, 2048);
        assert_eq!(header.total_chunks, 2);
    }

    #[test]
    fn test_parse_invalid_magic() {
        let mut header_bytes = create_valid_header(EncryptionAlgorithm::Aes256Gcm, 1024, 1);
        // 修改 magic 字节
        header_bytes[0] = 0x00;

        let mut cursor = Cursor::new(header_bytes);
        let result = FileHeaderParser::parse(&mut cursor);

        assert!(result.is_err());
        match result {
            Err(DecryptError::InvalidFormat(msg)) => {
                assert!(msg.contains("无效的文件魔数"));
            }
            _ => panic!("Expected InvalidFormat error"),
        }
    }

    #[test]
    fn test_parse_invalid_algorithm() {
        let mut header_bytes = create_valid_header(EncryptionAlgorithm::Aes256Gcm, 1024, 1);
        // 设置无效的算法标识
        header_bytes[6] = 255;

        let mut cursor = Cursor::new(header_bytes);
        let result = FileHeaderParser::parse(&mut cursor);

        assert!(result.is_err());
        match result {
            Err(DecryptError::InvalidFormat(msg)) => {
                assert!(msg.contains("无效的算法标识"));
            }
            _ => panic!("Expected InvalidFormat error"),
        }
    }

    #[test]
    fn test_parse_truncated_header() {
        // 只有 10 字节，不足 31 字节
        let header_bytes = vec![0xA3, 0x7F, 0x2C, 0x91, 0xE4, 0x5B, 0x00, 0x00, 0x00, 0x00];
        let mut cursor = Cursor::new(header_bytes);

        let result = FileHeaderParser::parse(&mut cursor);

        assert!(result.is_err());
        match result {
            Err(DecryptError::InvalidFormat(msg)) => {
                assert!(msg.contains("文件太小"));
            }
            _ => panic!("Expected InvalidFormat error"),
        }
    }

    #[test]
    fn test_parse_empty_file() {
        let header_bytes: Vec<u8> = vec![];
        let mut cursor = Cursor::new(header_bytes);

        let result = FileHeaderParser::parse(&mut cursor);

        assert!(result.is_err());
        match result {
            Err(DecryptError::InvalidFormat(msg)) => {
                assert!(msg.contains("文件太小"));
            }
            _ => panic!("Expected InvalidFormat error"),
        }
    }

    #[test]
    fn test_validate_magic_valid() {
        assert!(FileHeaderParser::validate_magic(&FILE_MAGIC));
    }

    #[test]
    fn test_validate_magic_invalid() {
        assert!(!FileHeaderParser::validate_magic(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00]));
        assert!(!FileHeaderParser::validate_magic(&[0xA3, 0x7F, 0x2C, 0x91, 0xE4, 0x00]));
    }

    #[test]
    fn test_chunk_reader_single_chunk() {
        // 创建一个包含单个分块的数据
        let mut data = Vec::new();
        // chunk_nonce (12 bytes)
        data.extend_from_slice(&[1u8; 12]);
        // ciphertext_len (4 bytes, little-endian)
        data.extend_from_slice(&16u32.to_le_bytes());
        // ciphertext (16 bytes)
        data.extend_from_slice(&[0xAB; 16]);

        let cursor = Cursor::new(data);
        let mut reader = ChunkReader::new(cursor, 1);

        assert!(reader.has_more());
        assert_eq!(reader.current_chunk(), 0);

        let chunk = reader.read_next_chunk().unwrap().unwrap();
        assert_eq!(chunk.nonce, [1u8; 12]);
        assert_eq!(chunk.ciphertext.len(), 16);
        assert_eq!(chunk.ciphertext, vec![0xAB; 16]);

        assert!(!reader.has_more());
        assert_eq!(reader.current_chunk(), 1);

        // 再次读取应该返回 None
        assert!(reader.read_next_chunk().unwrap().is_none());
    }

    #[test]
    fn test_chunk_reader_multiple_chunks() {
        let mut data = Vec::new();

        // 第一个分块
        data.extend_from_slice(&[1u8; 12]); // nonce
        data.extend_from_slice(&8u32.to_le_bytes()); // len
        data.extend_from_slice(&[0xAA; 8]); // ciphertext

        // 第二个分块
        data.extend_from_slice(&[2u8; 12]); // nonce
        data.extend_from_slice(&16u32.to_le_bytes()); // len
        data.extend_from_slice(&[0xBB; 16]); // ciphertext

        let cursor = Cursor::new(data);
        let mut reader = ChunkReader::new(cursor, 2);

        // 读取第一个分块
        let chunk1 = reader.read_next_chunk().unwrap().unwrap();
        assert_eq!(chunk1.nonce, [1u8; 12]);
        assert_eq!(chunk1.ciphertext, vec![0xAA; 8]);

        // 读取第二个分块
        let chunk2 = reader.read_next_chunk().unwrap().unwrap();
        assert_eq!(chunk2.nonce, [2u8; 12]);
        assert_eq!(chunk2.ciphertext, vec![0xBB; 16]);

        // 没有更多分块
        assert!(reader.read_next_chunk().unwrap().is_none());
    }

    #[test]
    fn test_chunk_reader_truncated_nonce() {
        // 只有 6 字节，不足 12 字节的 nonce
        let data = vec![1u8; 6];
        let cursor = Cursor::new(data);
        let mut reader = ChunkReader::new(cursor, 1);

        let result = reader.read_next_chunk();
        assert!(result.is_err());
        match result {
            Err(DecryptError::CorruptedFile(msg)) => {
                assert!(msg.contains("Nonce 数据不完整"));
            }
            _ => panic!("Expected CorruptedFile error"),
        }
    }

    #[test]
    fn test_chunk_reader_truncated_ciphertext() {
        let mut data = Vec::new();
        data.extend_from_slice(&[1u8; 12]); // nonce
        data.extend_from_slice(&100u32.to_le_bytes()); // len = 100
        data.extend_from_slice(&[0xAA; 50]); // 只有 50 字节，不足 100 字节

        let cursor = Cursor::new(data);
        let mut reader = ChunkReader::new(cursor, 1);

        let result = reader.read_next_chunk();
        assert!(result.is_err());
        match result {
            Err(DecryptError::CorruptedFile(msg)) => {
                assert!(msg.contains("密文数据不完整"));
            }
            _ => panic!("Expected CorruptedFile error"),
        }
    }

    #[test]
    fn test_chunk_reader_oversized_chunk() {
        let mut data = Vec::new();
        data.extend_from_slice(&[1u8; 12]); // nonce
        // 设置一个超大的长度值（超过 16MB + 16）
        data.extend_from_slice(&(20 * 1024 * 1024u32).to_le_bytes());

        let cursor = Cursor::new(data);
        let mut reader = ChunkReader::new(cursor, 1);

        let result = reader.read_next_chunk();
        assert!(result.is_err());
        match result {
            Err(DecryptError::CorruptedFile(msg)) => {
                assert!(msg.contains("密文长度异常"));
            }
            _ => panic!("Expected CorruptedFile error"),
        }
    }

    #[test]
    fn test_chunk_reader_iterator() {
        let mut data = Vec::new();

        // 两个分块
        for i in 0..2 {
            data.extend_from_slice(&[i as u8; 12]);
            data.extend_from_slice(&4u32.to_le_bytes());
            data.extend_from_slice(&[i as u8; 4]);
        }

        let cursor = Cursor::new(data);
        let reader = ChunkReader::new(cursor, 2);

        let chunks: Vec<_> = reader.collect();
        assert_eq!(chunks.len(), 2);

        let chunk0 = chunks[0].as_ref().unwrap();
        assert_eq!(chunk0.nonce, [0u8; 12]);

        let chunk1 = chunks[1].as_ref().unwrap();
        assert_eq!(chunk1.nonce, [1u8; 12]);
    }

    #[test]
    fn test_derive_chunk_nonce() {
        let master_nonce = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C];

        // chunk_index = 0
        let nonce0 = derive_chunk_nonce(&master_nonce, 0);
        assert_eq!(nonce0, master_nonce); // XOR with 0 should be unchanged

        // chunk_index = 1
        let nonce1 = derive_chunk_nonce(&master_nonce, 1);
        assert_eq!(nonce1[0..8], master_nonce[0..8]); // 前 8 字节不变
        assert_eq!(nonce1[8], 0x09 ^ 0x01); // 最后 4 字节 XOR
        assert_eq!(nonce1[9], 0x0A ^ 0x00);
        assert_eq!(nonce1[10], 0x0B ^ 0x00);
        assert_eq!(nonce1[11], 0x0C ^ 0x00);

        // chunk_index = 256
        let nonce256 = derive_chunk_nonce(&master_nonce, 256);
        assert_eq!(nonce256[8], 0x09 ^ 0x00);
        assert_eq!(nonce256[9], 0x0A ^ 0x01); // 256 = 0x00000100
    }

    #[test]
    fn test_large_file_header() {
        // 测试大文件（超过 4GB）
        let original_size: u64 = 5 * 1024 * 1024 * 1024; // 5GB
        let total_chunks: u32 = ((original_size as usize + 16 * 1024 * 1024 - 1) / (16 * 1024 * 1024)) as u32;

        let header_bytes = create_valid_header(EncryptionAlgorithm::Aes256Gcm, original_size, total_chunks);
        let mut cursor = Cursor::new(header_bytes);

        let header = FileHeaderParser::parse(&mut cursor).unwrap();

        assert_eq!(header.original_size, original_size);
        assert_eq!(header.total_chunks, total_chunks);
    }

    #[test]
    fn test_header_with_custom_nonce() {
        let mut header_bytes = Vec::with_capacity(FILE_HEADER_SIZE);
        header_bytes.extend_from_slice(&FILE_MAGIC);
        header_bytes.push(0); // AES-256-GCM
        // 自定义 nonce
        let custom_nonce = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC];
        header_bytes.extend_from_slice(&custom_nonce);
        header_bytes.extend_from_slice(&1024u64.to_le_bytes());
        header_bytes.extend_from_slice(&1u32.to_le_bytes());

        let mut cursor = Cursor::new(header_bytes);
        let header = FileHeaderParser::parse(&mut cursor).unwrap();

        assert_eq!(header.master_nonce, custom_nonce);
    }
}


// ============================================================================
// 属性测试 (Property-Based Tests)
// ============================================================================

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;
    use std::io::Cursor;

    /// 创建有效的文件头字节
    fn create_valid_header_bytes(
        algorithm: EncryptionAlgorithm,
        master_nonce: [u8; 12],
        original_size: u64,
        total_chunks: u32,
    ) -> Vec<u8> {
        let mut header = Vec::with_capacity(FILE_HEADER_SIZE);
        header.extend_from_slice(&FILE_MAGIC);
        header.push(algorithm.to_byte());
        header.extend_from_slice(&master_nonce);
        header.extend_from_slice(&original_size.to_le_bytes());
        header.extend_from_slice(&total_chunks.to_le_bytes());
        header
    }

    // ========================================================================
    // Property 5: 文件格式验证
    // 
    // *For any* 输入文件，如果文件头的 magic 字节不匹配预期值或文件数据损坏，
    // 解密操作必须返回退出码 3，不产生任何输出文件。
    //
    // **Validates: Requirements 4.3, 4.4, 6.4**
    // ========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 5.1: 无效的 magic 字节必须被检测并返回 InvalidFormat 错误
        ///
        /// **Validates: Requirements 4.3, 6.4**
        #[test]
        fn prop_invalid_magic_detected(
            // 生成随机的 6 字节 magic（排除有效的 magic）
            magic in prop::array::uniform6(any::<u8>())
                .prop_filter("must not be valid magic", |m| m != &FILE_MAGIC),
            algo in 0u8..=1u8,
            nonce in prop::array::uniform12(any::<u8>()),
            size in any::<u64>(),
            chunks in any::<u32>(),
        ) {
            // 构建带有无效 magic 的文件头
            let mut header = Vec::with_capacity(FILE_HEADER_SIZE);
            header.extend_from_slice(&magic);
            header.push(algo);
            header.extend_from_slice(&nonce);
            header.extend_from_slice(&size.to_le_bytes());
            header.extend_from_slice(&chunks.to_le_bytes());

            let mut cursor = Cursor::new(header);
            let result = FileHeaderParser::parse(&mut cursor);

            // 必须返回 InvalidFormat 错误
            prop_assert!(result.is_err(), "Invalid magic should be rejected");
            match result {
                Err(DecryptError::InvalidFormat(msg)) => {
                    prop_assert!(msg.contains("无效的文件魔数"), "Error message should mention invalid magic: {}", msg);
                }
                Err(e) => {
                    prop_assert!(false, "Expected InvalidFormat error, got: {:?}", e);
                }
                Ok(_) => {
                    prop_assert!(false, "Should not succeed with invalid magic");
                }
            }
        }

        /// Property 5.2: 无效的算法标识必须被检测并返回 InvalidFormat 错误
        ///
        /// **Validates: Requirements 4.3, 6.4**
        #[test]
        fn prop_invalid_algorithm_detected(
            // 生成无效的算法标识（2-255）
            algo in 2u8..=255u8,
            nonce in prop::array::uniform12(any::<u8>()),
            size in any::<u64>(),
            chunks in any::<u32>(),
        ) {
            // 构建带有无效算法标识的文件头
            let mut header = Vec::with_capacity(FILE_HEADER_SIZE);
            header.extend_from_slice(&FILE_MAGIC);
            header.push(algo);
            header.extend_from_slice(&nonce);
            header.extend_from_slice(&size.to_le_bytes());
            header.extend_from_slice(&chunks.to_le_bytes());

            let mut cursor = Cursor::new(header);
            let result = FileHeaderParser::parse(&mut cursor);

            // 必须返回 InvalidFormat 错误
            prop_assert!(result.is_err(), "Invalid algorithm should be rejected");
            match result {
                Err(DecryptError::InvalidFormat(msg)) => {
                    prop_assert!(msg.contains("无效的算法标识"), "Error message should mention invalid algorithm: {}", msg);
                }
                Err(e) => {
                    prop_assert!(false, "Expected InvalidFormat error, got: {:?}", e);
                }
                Ok(_) => {
                    prop_assert!(false, "Should not succeed with invalid algorithm");
                }
            }
        }

        /// Property 5.3: 截断的文件头必须被检测并返回 InvalidFormat 错误
        ///
        /// **Validates: Requirements 4.4, 6.4**
        #[test]
        fn prop_truncated_header_detected(
            // 生成 0 到 30 字节的截断数据（不足 31 字节）
            len in 0usize..FILE_HEADER_SIZE,
            data in prop::collection::vec(any::<u8>(), 0..FILE_HEADER_SIZE),
        ) {
            // 截取指定长度的数据
            let truncated: Vec<u8> = data.into_iter().take(len).collect();

            let mut cursor = Cursor::new(truncated);
            let result = FileHeaderParser::parse(&mut cursor);

            // 必须返回 InvalidFormat 错误
            prop_assert!(result.is_err(), "Truncated header should be rejected, len={}", len);
            match result {
                Err(DecryptError::InvalidFormat(msg)) => {
                    prop_assert!(msg.contains("文件太小"), "Error message should mention file too small: {}", msg);
                }
                Err(e) => {
                    prop_assert!(false, "Expected InvalidFormat error, got: {:?}", e);
                }
                Ok(_) => {
                    prop_assert!(false, "Should not succeed with truncated header");
                }
            }
        }

        /// Property 5.4: 有效的文件头必须被正确解析
        ///
        /// **Validates: Requirements 4.1, 4.2**
        #[test]
        fn prop_valid_header_parsed_correctly(
            algo in prop_oneof![Just(0u8), Just(1u8)],
            nonce in prop::array::uniform12(any::<u8>()),
            size in any::<u64>(),
            chunks in any::<u32>(),
        ) {
            let algorithm = EncryptionAlgorithm::from_byte(algo).unwrap();
            let header_bytes = create_valid_header_bytes(algorithm, nonce, size, chunks);

            let mut cursor = Cursor::new(header_bytes);
            let result = FileHeaderParser::parse(&mut cursor);

            // 必须成功解析
            prop_assert!(result.is_ok(), "Valid header should be parsed successfully");
            let header = result.unwrap();

            // 验证解析结果
            prop_assert!(header.validate_magic(), "Magic should be valid");
            prop_assert_eq!(header.algorithm, algorithm, "Algorithm should match");
            prop_assert_eq!(header.master_nonce, nonce, "Nonce should match");
            prop_assert_eq!(header.original_size, size, "Size should match");
            prop_assert_eq!(header.total_chunks, chunks, "Chunks should match");
        }

        /// Property 5.5: 分块数据截断必须被检测并返回 CorruptedFile 错误
        ///
        /// **Validates: Requirements 4.4, 6.4**
        #[test]
        fn prop_truncated_chunk_detected(
            // 生成有效的分块头但截断密文
            nonce in prop::array::uniform12(any::<u8>()),
            declared_len in 16u32..1024u32,  // 声明的长度
            actual_len in 0usize..15usize,   // 实际提供的长度（小于声明的长度）
        ) {
            // 构建截断的分块数据
            let mut data = Vec::new();
            data.extend_from_slice(&nonce);
            data.extend_from_slice(&declared_len.to_le_bytes());
            // 只提供部分密文
            data.extend(std::iter::repeat(0xAA).take(actual_len));

            let cursor = Cursor::new(data);
            let mut reader = ChunkReader::new(cursor, 1);

            let result = reader.read_next_chunk();

            // 必须返回 CorruptedFile 错误
            prop_assert!(result.is_err(), "Truncated chunk should be rejected");
            match result {
                Err(DecryptError::CorruptedFile(msg)) => {
                    prop_assert!(msg.contains("密文数据不完整"), "Error message should mention incomplete ciphertext: {}", msg);
                }
                Err(e) => {
                    prop_assert!(false, "Expected CorruptedFile error, got: {:?}", e);
                }
                Ok(_) => {
                    prop_assert!(false, "Should not succeed with truncated chunk");
                }
            }
        }

        /// Property 5.6: 超大分块长度必须被检测并返回 CorruptedFile 错误
        ///
        /// **Validates: Requirements 4.4, 6.4**
        #[test]
        fn prop_oversized_chunk_detected(
            nonce in prop::array::uniform12(any::<u8>()),
            // 生成超过最大允许大小的长度值
            oversized_len in (16 * 1024 * 1024 + 17)..u32::MAX,
        ) {
            // 构建带有超大长度的分块数据
            let mut data = Vec::new();
            data.extend_from_slice(&nonce);
            data.extend_from_slice(&oversized_len.to_le_bytes());

            let cursor = Cursor::new(data);
            let mut reader = ChunkReader::new(cursor, 1);

            let result = reader.read_next_chunk();

            // 必须返回 CorruptedFile 错误
            prop_assert!(result.is_err(), "Oversized chunk should be rejected");
            match result {
                Err(DecryptError::CorruptedFile(msg)) => {
                    prop_assert!(msg.contains("密文长度异常"), "Error message should mention abnormal length: {}", msg);
                }
                Err(e) => {
                    prop_assert!(false, "Expected CorruptedFile error, got: {:?}", e);
                }
                Ok(_) => {
                    prop_assert!(false, "Should not succeed with oversized chunk");
                }
            }
        }

        /// Property 5.7: 有效的分块数据必须被正确读取
        ///
        /// **Validates: Requirements 4.1, 4.2**
        #[test]
        fn prop_valid_chunk_read_correctly(
            nonce in prop::array::uniform12(any::<u8>()),
            // 生成合理大小的密文（16 字节到 1KB）
            ciphertext in prop::collection::vec(any::<u8>(), 16..1024),
        ) {
            // 构建有效的分块数据
            let mut data = Vec::new();
            data.extend_from_slice(&nonce);
            data.extend_from_slice(&(ciphertext.len() as u32).to_le_bytes());
            data.extend_from_slice(&ciphertext);

            let cursor = Cursor::new(data);
            let mut reader = ChunkReader::new(cursor, 1);

            let result = reader.read_next_chunk();

            // 必须成功读取
            prop_assert!(result.is_ok(), "Valid chunk should be read successfully");
            let chunk = result.unwrap().unwrap();

            // 验证读取结果
            prop_assert_eq!(chunk.nonce, nonce, "Nonce should match");
            prop_assert_eq!(chunk.ciphertext, ciphertext, "Ciphertext should match");
        }

        /// Property 5.8: derive_chunk_nonce 必须是确定性的
        ///
        /// **Validates: Requirements 4.1**
        #[test]
        fn prop_derive_chunk_nonce_deterministic(
            master_nonce in prop::array::uniform12(any::<u8>()),
            chunk_index in any::<u32>(),
        ) {
            let nonce1 = derive_chunk_nonce(&master_nonce, chunk_index);
            let nonce2 = derive_chunk_nonce(&master_nonce, chunk_index);

            prop_assert_eq!(nonce1, nonce2, "derive_chunk_nonce should be deterministic");
        }

        /// Property 5.9: 不同的 chunk_index 应该产生不同的 nonce（除非 master_nonce 特殊）
        ///
        /// **Validates: Requirements 4.1**
        #[test]
        fn prop_different_chunk_index_different_nonce(
            master_nonce in prop::array::uniform12(any::<u8>()),
            index1 in any::<u32>(),
            index2 in any::<u32>(),
        ) {
            prop_assume!(index1 != index2);

            let nonce1 = derive_chunk_nonce(&master_nonce, index1);
            let nonce2 = derive_chunk_nonce(&master_nonce, index2);

            // 由于 XOR 操作，不同的 index 应该产生不同的 nonce
            // 除非 master_nonce 的最后 4 字节恰好使得 XOR 结果相同（极其罕见）
            // 我们只验证当 index 不同时，nonce 通常不同
            if nonce1 == nonce2 {
                // 这种情况只有在 master_nonce 的最后 4 字节与两个 index 的 XOR 结果相同时才会发生
                // 这是一个非常罕见的情况，我们允许它发生但记录下来
                let xor1 = index1 ^ u32::from_le_bytes([master_nonce[8], master_nonce[9], master_nonce[10], master_nonce[11]]);
                let xor2 = index2 ^ u32::from_le_bytes([master_nonce[8], master_nonce[9], master_nonce[10], master_nonce[11]]);
                prop_assert_eq!(xor1, xor2, "Nonces should only be equal if XOR results are equal");
            }
        }
    }
}
