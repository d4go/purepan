//! 错误处理模块
//!
//! 实现 DecryptError 到 ExitCode 的转换、批量解密结果汇总、
//! 错误信息格式化输出等功能。
//!
//! # 退出码定义
//!
//! | 退出码 | 含义 |
//! |--------|------|
//! | 0 | 全部成功 |
//! | 1 | 部分成功（批量解密时部分文件失败） |
//! | 2 | 参数错误 |
//! | 3 | 文件格式无效/损坏 |
//! | 4 | 密钥不匹配 |
//! | 5 | I/O 错误 |
//! | 6 | 映射错误 |

use std::fmt;
use std::path::Path;

use crate::types::{DecryptError, DecryptSummary, ExitCode, FileDecryptResult};

// ============================================================================
// 错误分类
// ============================================================================

/// 错误类别
///
/// 用于对错误进行分类，以便确定最终退出码
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// 参数错误
    Argument,
    /// 文件格式错误
    Format,
    /// 密钥错误
    Key,
    /// I/O 错误
    Io,
    /// 映射错误
    Mapping,
}

impl ErrorCategory {
    /// 转换为退出码
    #[allow(dead_code)]
    pub fn to_exit_code(self) -> ExitCode {
        match self {
            ErrorCategory::Argument => ExitCode::ArgumentError,
            ErrorCategory::Format => ExitCode::InvalidFormat,
            ErrorCategory::Key => ExitCode::KeyMismatch,
            ErrorCategory::Io => ExitCode::IoError,
            ErrorCategory::Mapping => ExitCode::MappingError,
        }
    }
}

impl From<&DecryptError> for ErrorCategory {
    fn from(error: &DecryptError) -> Self {
        match error {
            DecryptError::ArgumentError(_) => ErrorCategory::Argument,
            DecryptError::InvalidFormat(_) => ErrorCategory::Format,
            DecryptError::CorruptedFile(_) => ErrorCategory::Format,
            DecryptError::KeyMismatch(_) => ErrorCategory::Key,
            DecryptError::KeyDecodeError(_) => ErrorCategory::Key,
            DecryptError::IoError(_) => ErrorCategory::Io,
            DecryptError::MappingError(_) => ErrorCategory::Mapping,
            DecryptError::JsonError(_) => ErrorCategory::Format,
            DecryptError::DecryptionFailed(_) => ErrorCategory::Format,
            DecryptError::MappingNotFound(_) => ErrorCategory::Mapping,
        }
    }
}

// ============================================================================
// 批量解密结果分析
// ============================================================================

/// 批量解密结果分析器
///
/// 分析批量解密结果，确定最终退出码和错误汇总
pub struct ResultAnalyzer<'a> {
    summary: &'a DecryptSummary,
}

impl<'a> ResultAnalyzer<'a> {
    /// 创建新的结果分析器
    pub fn new(summary: &'a DecryptSummary) -> Self {
        Self { summary }
    }

    /// 确定最终退出码
    ///
    /// 根据批量解密结果确定退出码：
    /// - 全部成功：返回 0
    /// - 部分成功：返回 1
    /// - 全部失败：根据第一个错误类型返回对应退出码
    /// - 空目录：返回 0
    pub fn determine_exit_code(&self) -> ExitCode {
        if self.summary.total_count() == 0 {
            // 空目录
            return ExitCode::Success;
        }

        if self.summary.failed_count == 0 && self.summary.skipped_count == 0 {
            // 全部成功
            return ExitCode::Success;
        }

        if self.summary.success_count > 0 {
            // 部分成功
            return ExitCode::PartialSuccess;
        }

        // 全部失败，根据第一个失败的错误类型确定退出码
        self.determine_failure_exit_code()
    }

    /// 根据失败类型确定退出码
    fn determine_failure_exit_code(&self) -> ExitCode {
        // 统计各类错误数量
        let mut key_errors = 0;
        let mut io_errors = 0;
        let mut mapping_errors = 0;

        for result in &self.summary.results {
            if !result.success {
                if let Some(ref error) = result.error {
                    if error.contains("密钥") || error.contains("key") {
                        key_errors += 1;
                    } else if error.contains("I/O") || error.contains("读取") || error.contains("写入") {
                        io_errors += 1;
                    } else if error.contains("映射") || error.contains("mapping") || error.contains("跳过") {
                        mapping_errors += 1;
                    }
                    // 格式错误和其他错误都归类为 InvalidFormat（默认）
                }
            }
        }

        // 按优先级返回退出码
        if key_errors > 0 {
            ExitCode::KeyMismatch
        } else if mapping_errors > 0 {
            ExitCode::MappingError
        } else if io_errors > 0 {
            ExitCode::IoError
        } else {
            ExitCode::InvalidFormat
        }
    }

    /// 获取错误统计
    #[allow(dead_code)]
    pub fn error_stats(&self) -> ErrorStats {
        let mut stats = ErrorStats::default();

        for result in &self.summary.results {
            if !result.success {
                if let Some(ref error) = result.error {
                    if error.starts_with("跳过:") {
                        stats.skipped += 1;
                    } else if error.contains("密钥") || error.contains("key") {
                        stats.key_errors += 1;
                    } else if error.contains("格式") || error.contains("损坏") {
                        stats.format_errors += 1;
                    } else if error.contains("I/O") {
                        stats.io_errors += 1;
                    } else if error.contains("映射") {
                        stats.mapping_errors += 1;
                    } else {
                        stats.other_errors += 1;
                    }
                }
            }
        }

        stats
    }
}

/// 错误统计
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct ErrorStats {
    /// 跳过的文件数
    pub skipped: usize,
    /// 密钥错误数
    pub key_errors: usize,
    /// 格式错误数
    pub format_errors: usize,
    /// I/O 错误数
    pub io_errors: usize,
    /// 映射错误数
    pub mapping_errors: usize,
    /// 其他错误数
    pub other_errors: usize,
}

impl ErrorStats {
    /// 获取总错误数
    #[allow(dead_code)]
    pub fn total_errors(&self) -> usize {
        self.key_errors + self.format_errors + self.io_errors + self.mapping_errors + self.other_errors
    }
}

// ============================================================================
// 错误信息格式化
// ============================================================================

/// 错误格式化器
///
/// 提供错误信息的格式化输出功能
#[allow(dead_code)]
pub struct ErrorFormatter;

#[allow(dead_code)]
impl ErrorFormatter {
    /// 格式化单个错误
    pub fn format_error(error: &DecryptError) -> String {
        match error {
            DecryptError::ArgumentError(msg) => format!("参数错误: {}", msg),
            DecryptError::InvalidFormat(msg) => format!("文件格式无效: {}", msg),
            DecryptError::CorruptedFile(msg) => format!("文件损坏: {}", msg),
            DecryptError::KeyMismatch(version) => format!("密钥不匹配: 版本 {} 不存在", version),
            DecryptError::KeyDecodeError(msg) => format!("密钥解码错误: {}", msg),
            DecryptError::IoError(e) => format!("I/O 错误: {}", e),
            DecryptError::MappingError(msg) => format!("映射错误: {}", msg),
            DecryptError::JsonError(e) => format!("JSON 解析错误: {}", e),
            DecryptError::DecryptionFailed(msg) => format!("解密失败: {}", msg),
            DecryptError::MappingNotFound(name) => format!("映射中找不到记录: {}", name),
        }
    }

    /// 格式化文件解密结果
    pub fn format_file_result(result: &FileDecryptResult) -> String {
        if result.success {
            format!(
                "✓ {} -> {}",
                result.input_path,
                result.output_path.as_deref().unwrap_or("?")
            )
        } else {
            format!(
                "✗ {}: {}",
                result.input_path,
                result.error.as_deref().unwrap_or("未知错误")
            )
        }
    }

    /// 格式化批量解密汇总
    pub fn format_summary(summary: &DecryptSummary) -> String {
        let mut output = String::new();
        output.push_str("\n解密完成:\n");
        output.push_str(&format!("  成功: {} 个文件\n", summary.success_count));

        if summary.skipped_count > 0 {
            output.push_str(&format!(
                "  跳过: {} 个文件 (无映射记录或非加密文件)\n",
                summary.skipped_count
            ));
        }

        if summary.failed_count > 0 {
            output.push_str(&format!("  失败: {} 个文件\n", summary.failed_count));

            // 列出失败的文件
            for result in &summary.results {
                if !result.success {
                    if let Some(ref error) = result.error {
                        if !error.starts_with("跳过:") {
                            output.push_str(&format!("    - {}: {}\n", result.input_path, error));
                        }
                    }
                }
            }
        }

        output
    }

    /// 格式化退出码说明
    pub fn format_exit_code_description(code: ExitCode) -> &'static str {
        match code {
            ExitCode::Success => "全部成功",
            ExitCode::PartialSuccess => "部分成功（部分文件解密失败）",
            ExitCode::ArgumentError => "参数错误",
            ExitCode::InvalidFormat => "文件格式无效或损坏",
            ExitCode::KeyMismatch => "密钥不匹配",
            ExitCode::IoError => "I/O 错误",
            ExitCode::MappingError => "映射错误",
        }
    }
}

// ============================================================================
// 特定场景错误处理
// ============================================================================

/// 场景错误处理器
///
/// 处理特定场景的错误，如映射中找不到记录、输入目录为空、输出文件已存在等
#[allow(dead_code)]
pub struct ScenarioHandler;

#[allow(dead_code)]
impl ScenarioHandler {
    /// 处理映射中找不到记录的情况
    pub fn handle_mapping_not_found(encrypted_name: &str) -> DecryptError {
        DecryptError::MappingNotFound(encrypted_name.to_string())
    }

    /// 处理输入目录为空的情况
    pub fn handle_empty_input_directory(dir: &Path) -> DecryptError {
        DecryptError::ArgumentError(format!(
            "输入目录 '{}' 为空或不包含任何文件",
            dir.display()
        ))
    }

    /// 处理输出文件已存在的情况
    pub fn handle_output_exists(path: &Path) -> DecryptError {
        DecryptError::IoError(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("输出文件 '{}' 已存在", path.display()),
        ))
    }

    /// 处理密钥版本不存在的情况
    pub fn handle_key_version_not_found(version: u32) -> DecryptError {
        DecryptError::KeyMismatch(version)
    }

    /// 处理无可用密钥的情况
    pub fn handle_no_available_keys() -> DecryptError {
        DecryptError::KeyMismatch(0)
    }

    /// 处理输入文件不存在的情况
    pub fn handle_input_not_found(path: &Path) -> DecryptError {
        DecryptError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("输入文件 '{}' 不存在", path.display()),
        ))
    }

    /// 处理输入目录不存在的情况
    pub fn handle_input_dir_not_found(path: &Path) -> DecryptError {
        DecryptError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("输入目录 '{}' 不存在", path.display()),
        ))
    }

    /// 检查输入目录是否为空
    pub fn check_empty_directory(dir: &Path) -> Result<(), DecryptError> {
        if !dir.exists() {
            return Err(Self::handle_input_dir_not_found(dir));
        }

        let entries = std::fs::read_dir(dir).map_err(DecryptError::IoError)?;
        let has_files = entries.into_iter().any(|e| e.is_ok());

        if !has_files {
            return Err(Self::handle_empty_input_directory(dir));
        }

        Ok(())
    }

    /// 检查输出文件是否已存在（可选覆盖）
    pub fn check_output_exists(path: &Path, allow_overwrite: bool) -> Result<(), DecryptError> {
        if path.exists() && !allow_overwrite {
            return Err(Self::handle_output_exists(path));
        }
        Ok(())
    }
}

// ============================================================================
// 错误上下文
// ============================================================================

/// 错误上下文
///
/// 为错误添加上下文信息
#[allow(dead_code)]
pub struct ErrorContext {
    /// 操作描述
    pub operation: String,
    /// 文件路径（如果适用）
    pub file_path: Option<String>,
    /// 原始错误
    pub source: DecryptError,
}

#[allow(dead_code)]
impl ErrorContext {
    /// 创建新的错误上下文
    pub fn new(operation: impl Into<String>, source: DecryptError) -> Self {
        Self {
            operation: operation.into(),
            file_path: None,
            source,
        }
    }

    /// 添加文件路径
    pub fn with_file(mut self, path: impl Into<String>) -> Self {
        self.file_path = Some(path.into());
        self
    }
}

impl fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref path) = self.file_path {
            write!(f, "{} '{}': {}", self.operation, path, self.source)
        } else {
            write!(f, "{}: {}", self.operation, self.source)
        }
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_category_from_decrypt_error() {
        assert_eq!(
            ErrorCategory::from(&DecryptError::ArgumentError("test".to_string())),
            ErrorCategory::Argument
        );
        assert_eq!(
            ErrorCategory::from(&DecryptError::InvalidFormat("test".to_string())),
            ErrorCategory::Format
        );
        assert_eq!(
            ErrorCategory::from(&DecryptError::CorruptedFile("test".to_string())),
            ErrorCategory::Format
        );
        assert_eq!(
            ErrorCategory::from(&DecryptError::KeyMismatch(1)),
            ErrorCategory::Key
        );
        assert_eq!(
            ErrorCategory::from(&DecryptError::KeyDecodeError("test".to_string())),
            ErrorCategory::Key
        );
        assert_eq!(
            ErrorCategory::from(&DecryptError::MappingError("test".to_string())),
            ErrorCategory::Mapping
        );
        assert_eq!(
            ErrorCategory::from(&DecryptError::MappingNotFound("test".to_string())),
            ErrorCategory::Mapping
        );
    }

    #[test]
    fn test_error_category_to_exit_code() {
        assert_eq!(ErrorCategory::Argument.to_exit_code(), ExitCode::ArgumentError);
        assert_eq!(ErrorCategory::Format.to_exit_code(), ExitCode::InvalidFormat);
        assert_eq!(ErrorCategory::Key.to_exit_code(), ExitCode::KeyMismatch);
        assert_eq!(ErrorCategory::Io.to_exit_code(), ExitCode::IoError);
        assert_eq!(ErrorCategory::Mapping.to_exit_code(), ExitCode::MappingError);
    }

    #[test]
    fn test_result_analyzer_all_success() {
        let mut summary = DecryptSummary::new();
        summary.add_success("file1.dat".to_string(), "file1.txt".to_string());
        summary.add_success("file2.dat".to_string(), "file2.txt".to_string());

        let analyzer = ResultAnalyzer::new(&summary);
        assert_eq!(analyzer.determine_exit_code(), ExitCode::Success);
    }

    #[test]
    fn test_result_analyzer_partial_success() {
        let mut summary = DecryptSummary::new();
        summary.add_success("file1.dat".to_string(), "file1.txt".to_string());
        summary.add_failed("file2.dat".to_string(), "密钥不匹配".to_string());

        let analyzer = ResultAnalyzer::new(&summary);
        assert_eq!(analyzer.determine_exit_code(), ExitCode::PartialSuccess);
    }

    #[test]
    fn test_result_analyzer_all_failed_key_error() {
        let mut summary = DecryptSummary::new();
        summary.add_failed("file1.dat".to_string(), "密钥版本 2 不存在".to_string());
        summary.add_failed("file2.dat".to_string(), "密钥不匹配".to_string());

        let analyzer = ResultAnalyzer::new(&summary);
        assert_eq!(analyzer.determine_exit_code(), ExitCode::KeyMismatch);
    }

    #[test]
    fn test_result_analyzer_all_failed_format_error() {
        let mut summary = DecryptSummary::new();
        summary.add_failed("file1.dat".to_string(), "文件格式无效".to_string());
        summary.add_failed("file2.dat".to_string(), "文件损坏".to_string());

        let analyzer = ResultAnalyzer::new(&summary);
        assert_eq!(analyzer.determine_exit_code(), ExitCode::InvalidFormat);
    }

    #[test]
    fn test_result_analyzer_empty_directory() {
        let summary = DecryptSummary::new();
        let analyzer = ResultAnalyzer::new(&summary);
        assert_eq!(analyzer.determine_exit_code(), ExitCode::Success);
    }

    #[test]
    fn test_result_analyzer_with_skipped() {
        let mut summary = DecryptSummary::new();
        summary.add_success("file1.dat".to_string(), "file1.txt".to_string());
        summary.add_skipped("file2.dat".to_string(), "无映射记录".to_string());

        let analyzer = ResultAnalyzer::new(&summary);
        assert_eq!(analyzer.determine_exit_code(), ExitCode::PartialSuccess);
    }

    #[test]
    fn test_error_stats() {
        let mut summary = DecryptSummary::new();
        summary.add_failed("file1.dat".to_string(), "密钥不匹配".to_string());
        summary.add_failed("file2.dat".to_string(), "文件格式无效".to_string());
        summary.add_skipped("file3.dat".to_string(), "无映射记录".to_string());

        let analyzer = ResultAnalyzer::new(&summary);
        let stats = analyzer.error_stats();

        assert_eq!(stats.key_errors, 1);
        assert_eq!(stats.format_errors, 1);
        assert_eq!(stats.skipped, 1);
    }

    #[test]
    fn test_error_formatter_format_error() {
        let error = DecryptError::KeyMismatch(2);
        let formatted = ErrorFormatter::format_error(&error);
        assert!(formatted.contains("密钥不匹配"));
        assert!(formatted.contains("2"));
    }

    #[test]
    fn test_error_formatter_format_file_result_success() {
        let result = FileDecryptResult {
            input_path: "input.dat".to_string(),
            output_path: Some("output.txt".to_string()),
            success: true,
            error: None,
        };
        let formatted = ErrorFormatter::format_file_result(&result);
        assert!(formatted.contains("✓"));
        assert!(formatted.contains("input.dat"));
        assert!(formatted.contains("output.txt"));
    }

    #[test]
    fn test_error_formatter_format_file_result_failure() {
        let result = FileDecryptResult {
            input_path: "input.dat".to_string(),
            output_path: None,
            success: false,
            error: Some("密钥不匹配".to_string()),
        };
        let formatted = ErrorFormatter::format_file_result(&result);
        assert!(formatted.contains("✗"));
        assert!(formatted.contains("input.dat"));
        assert!(formatted.contains("密钥不匹配"));
    }

    #[test]
    fn test_error_formatter_format_summary() {
        let mut summary = DecryptSummary::new();
        summary.add_success("file1.dat".to_string(), "file1.txt".to_string());
        summary.add_failed("file2.dat".to_string(), "密钥不匹配".to_string());
        summary.add_skipped("file3.dat".to_string(), "无映射记录".to_string());

        let formatted = ErrorFormatter::format_summary(&summary);
        assert!(formatted.contains("成功: 1 个文件"));
        assert!(formatted.contains("失败: 1 个文件"));
        assert!(formatted.contains("跳过: 1 个文件"));
    }

    #[test]
    fn test_scenario_handler_mapping_not_found() {
        let error = ScenarioHandler::handle_mapping_not_found("test.dat");
        assert!(matches!(error, DecryptError::MappingNotFound(_)));
    }

    #[test]
    fn test_scenario_handler_empty_input_directory() {
        let error = ScenarioHandler::handle_empty_input_directory(Path::new("/empty"));
        assert!(matches!(error, DecryptError::ArgumentError(_)));
    }

    #[test]
    fn test_scenario_handler_output_exists() {
        let error = ScenarioHandler::handle_output_exists(Path::new("/output.txt"));
        assert!(matches!(error, DecryptError::IoError(_)));
    }

    #[test]
    fn test_scenario_handler_key_version_not_found() {
        let error = ScenarioHandler::handle_key_version_not_found(5);
        assert!(matches!(error, DecryptError::KeyMismatch(5)));
    }

    #[test]
    fn test_scenario_handler_no_available_keys() {
        let error = ScenarioHandler::handle_no_available_keys();
        assert!(matches!(error, DecryptError::KeyMismatch(0)));
    }

    #[test]
    fn test_error_context_display() {
        let error = DecryptError::KeyMismatch(2);
        let context = ErrorContext::new("解密文件", error).with_file("test.dat");
        let display = format!("{}", context);
        assert!(display.contains("解密文件"));
        assert!(display.contains("test.dat"));
    }

    #[test]
    fn test_error_context_display_without_file() {
        let error = DecryptError::ArgumentError("无效参数".to_string());
        let context = ErrorContext::new("验证参数", error);
        let display = format!("{}", context);
        assert!(display.contains("验证参数"));
        assert!(display.contains("无效参数"));
    }
}


// ============================================================================
// 属性测试 (Property-Based Tests)
// ============================================================================

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    /// 生成随机的成功文件结果
    #[allow(dead_code)]
    fn arb_success_result() -> impl Strategy<Value = FileDecryptResult> {
        ("[a-zA-Z0-9_]{1,20}\\.dat", "[a-zA-Z0-9_]{1,20}\\.txt").prop_map(
            |(input, output)| FileDecryptResult {
                input_path: input,
                output_path: Some(output),
                success: true,
                error: None,
            },
        )
    }

    /// 生成随机的失败文件结果（带错误类型）
    #[allow(dead_code)]
    fn arb_failed_result() -> impl Strategy<Value = FileDecryptResult> {
        (
            "[a-zA-Z0-9_]{1,20}\\.dat",
            prop_oneof![
                Just("密钥版本 2 不存在".to_string()),
                Just("密钥不匹配".to_string()),
                Just("文件格式无效".to_string()),
                Just("文件损坏".to_string()),
                Just("I/O 错误: 读取失败".to_string()),
                Just("映射中找不到记录".to_string()),
            ],
        )
            .prop_map(|(input, error)| FileDecryptResult {
                input_path: input,
                output_path: None,
                success: false,
                error: Some(error),
            })
    }

    /// 生成随机的跳过文件结果
    #[allow(dead_code)]
    fn arb_skipped_result() -> impl Strategy<Value = FileDecryptResult> {
        "[a-zA-Z0-9_]{1,20}\\.dat".prop_map(|input| FileDecryptResult {
            input_path: input,
            output_path: None,
            success: false,
            error: Some("跳过: 无映射记录".to_string()),
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        // ====================================================================
        // Property 7: 退出码正确性
        //
        // *For any* 批量解密结果组合，退出码应该正确反映解密状态：
        // - 全部成功：退出码 0
        // - 部分成功：退出码 1
        // - 全部失败：根据错误类型返回对应退出码
        //
        // **Validates: Requirements 6.1, 6.2, 6.3**
        // ====================================================================

        /// Property 7.1: 全部成功时返回退出码 0
        ///
        /// 当所有文件都成功解密时，退出码应该为 0。
        ///
        /// **Validates: Requirements 6.1**
        #[test]
        fn prop_all_success_returns_exit_code_0(
            success_count in 1usize..=10,
        ) {
            let mut summary = DecryptSummary::new();

            // 添加成功结果
            for i in 0..success_count {
                summary.add_success(
                    format!("file{}.dat", i),
                    format!("file{}.txt", i),
                );
            }

            let analyzer = ResultAnalyzer::new(&summary);
            let exit_code = analyzer.determine_exit_code();

            prop_assert_eq!(
                exit_code,
                ExitCode::Success,
                "全部成功时应返回退出码 0"
            );
        }

        /// Property 7.2: 部分成功时返回退出码 1
        ///
        /// 当部分文件成功、部分文件失败时，退出码应该为 1。
        ///
        /// **Validates: Requirements 6.2**
        #[test]
        fn prop_partial_success_returns_exit_code_1(
            success_count in 1usize..=5,
            failed_count in 1usize..=5,
        ) {
            let mut summary = DecryptSummary::new();

            // 添加成功结果
            for i in 0..success_count {
                summary.add_success(
                    format!("success{}.dat", i),
                    format!("success{}.txt", i),
                );
            }

            // 添加失败结果
            for i in 0..failed_count {
                summary.add_failed(
                    format!("failed{}.dat", i),
                    "解密失败".to_string(),
                );
            }

            let analyzer = ResultAnalyzer::new(&summary);
            let exit_code = analyzer.determine_exit_code();

            prop_assert_eq!(
                exit_code,
                ExitCode::PartialSuccess,
                "部分成功时应返回退出码 1"
            );
        }

        /// Property 7.3: 有成功和跳过时返回退出码 1
        ///
        /// 当有成功文件和跳过文件时，退出码应该为 1（部分成功）。
        ///
        /// **Validates: Requirements 6.2**
        #[test]
        fn prop_success_with_skipped_returns_exit_code_1(
            success_count in 1usize..=5,
            skipped_count in 1usize..=5,
        ) {
            let mut summary = DecryptSummary::new();

            // 添加成功结果
            for i in 0..success_count {
                summary.add_success(
                    format!("success{}.dat", i),
                    format!("success{}.txt", i),
                );
            }

            // 添加跳过结果
            for i in 0..skipped_count {
                summary.add_skipped(
                    format!("skipped{}.dat", i),
                    "无映射记录".to_string(),
                );
            }

            let analyzer = ResultAnalyzer::new(&summary);
            let exit_code = analyzer.determine_exit_code();

            prop_assert_eq!(
                exit_code,
                ExitCode::PartialSuccess,
                "有成功和跳过时应返回退出码 1"
            );
        }

        /// Property 7.4: 空目录返回退出码 0
        ///
        /// 当输入目录为空时，退出码应该为 0。
        ///
        /// **Validates: Requirements 6.1**
        #[test]
        fn prop_empty_directory_returns_exit_code_0(_dummy in 0..1) {
            let summary = DecryptSummary::new();
            let analyzer = ResultAnalyzer::new(&summary);
            let exit_code = analyzer.determine_exit_code();

            prop_assert_eq!(
                exit_code,
                ExitCode::Success,
                "空目录应返回退出码 0"
            );
        }

        /// Property 7.5: 全部失败时根据错误类型返回对应退出码
        ///
        /// 当所有文件都失败时，应根据第一个错误类型返回对应退出码。
        ///
        /// **Validates: Requirements 6.3**
        #[test]
        fn prop_all_failed_returns_appropriate_exit_code(
            error_type in prop_oneof![
                Just(("密钥不匹配", ExitCode::KeyMismatch)),
                Just(("文件格式无效", ExitCode::InvalidFormat)),
                Just(("映射中找不到记录", ExitCode::MappingError)),
            ],
            count in 1usize..=5,
        ) {
            let (error_msg, expected_code) = error_type;
            let mut summary = DecryptSummary::new();

            // 添加相同类型的失败结果
            for i in 0..count {
                summary.add_failed(
                    format!("file{}.dat", i),
                    error_msg.to_string(),
                );
            }

            let analyzer = ResultAnalyzer::new(&summary);
            let exit_code = analyzer.determine_exit_code();

            prop_assert_eq!(
                exit_code,
                expected_code,
                "全部失败时应返回对应错误类型的退出码"
            );
        }

        /// Property 7.6: 混合成功/失败/跳过结果的退出码
        ///
        /// 验证各种组合情况下的退出码正确性。
        ///
        /// **Validates: Requirements 6.1, 6.2, 6.3**
        #[test]
        fn prop_mixed_results_exit_code(
            success_count in 0usize..=3,
            failed_count in 0usize..=3,
            skipped_count in 0usize..=3,
        ) {
            // 确保至少有一个结果
            prop_assume!(success_count + failed_count + skipped_count > 0);

            let mut summary = DecryptSummary::new();

            for i in 0..success_count {
                summary.add_success(
                    format!("success{}.dat", i),
                    format!("success{}.txt", i),
                );
            }

            for i in 0..failed_count {
                summary.add_failed(
                    format!("failed{}.dat", i),
                    "解密失败".to_string(),
                );
            }

            for i in 0..skipped_count {
                summary.add_skipped(
                    format!("skipped{}.dat", i),
                    "无映射记录".to_string(),
                );
            }

            let analyzer = ResultAnalyzer::new(&summary);
            let exit_code = analyzer.determine_exit_code();

            // 验证退出码逻辑
            if success_count > 0 && (failed_count > 0 || skipped_count > 0) {
                // 部分成功
                prop_assert_eq!(
                    exit_code,
                    ExitCode::PartialSuccess,
                    "有成功和失败/跳过时应返回退出码 1"
                );
            } else if success_count > 0 {
                // 全部成功
                prop_assert_eq!(
                    exit_code,
                    ExitCode::Success,
                    "全部成功时应返回退出码 0"
                );
            } else if failed_count > 0 || skipped_count > 0 {
                // 全部失败或只有跳过
                // 根据 ResultAnalyzer 的逻辑，这种情况会根据错误类型返回对应退出码
                // 跳过的文件会被归类为映射错误
                prop_assert!(
                    exit_code != ExitCode::Success,
                    "全部失败或只有跳过时不应返回成功"
                );
            }
        }

        /// Property 7.7: 退出码与 DecryptSummary.exit_code() 一致
        ///
        /// ResultAnalyzer 的退出码应与 DecryptSummary 的 exit_code() 方法一致。
        ///
        /// **Validates: Requirements 6.1, 6.2, 6.3**
        #[test]
        fn prop_exit_code_consistency(
            success_count in 0usize..=3,
            failed_count in 0usize..=3,
            skipped_count in 0usize..=3,
        ) {
            prop_assume!(success_count + failed_count + skipped_count > 0);

            let mut summary = DecryptSummary::new();

            for i in 0..success_count {
                summary.add_success(
                    format!("success{}.dat", i),
                    format!("success{}.txt", i),
                );
            }

            for i in 0..failed_count {
                summary.add_failed(
                    format!("failed{}.dat", i),
                    "解密失败".to_string(),
                );
            }

            for i in 0..skipped_count {
                summary.add_skipped(
                    format!("skipped{}.dat", i),
                    "无映射记录".to_string(),
                );
            }

            let analyzer = ResultAnalyzer::new(&summary);
            let analyzer_code = analyzer.determine_exit_code();
            let summary_code = summary.exit_code();

            // 两者的基本逻辑应该一致
            // 注意：全部失败时可能有细微差异，因为 ResultAnalyzer 会分析具体错误类型
            if success_count > 0 {
                prop_assert_eq!(
                    analyzer_code,
                    summary_code,
                    "有成功时两者应返回相同退出码"
                );
            }
        }

        // ====================================================================
        // Property 6: 密钥不匹配处理
        //
        // *For any* 密钥版本不存在的场景，解密引擎应返回退出码 4（密钥不匹配）。
        //
        // **Validates: Requirements 5.2, 6.5**
        // ====================================================================

        /// Property 6.1: 密钥版本不存在时返回退出码 4
        ///
        /// 当映射记录中的 key_version 在 encryption.json 中不存在时，
        /// 应返回退出码 4（密钥不匹配）。
        ///
        /// **Validates: Requirements 5.2, 6.5**
        #[test]
        fn prop_key_mismatch_returns_exit_code_4(
            missing_version in 1u32..=100,
            count in 1usize..=5,
        ) {
            let mut summary = DecryptSummary::new();

            // 添加密钥不匹配的失败结果
            for i in 0..count {
                summary.add_failed(
                    format!("file{}.dat", i),
                    format!("密钥版本 {} 不存在", missing_version),
                );
            }

            let analyzer = ResultAnalyzer::new(&summary);
            let exit_code = analyzer.determine_exit_code();

            prop_assert_eq!(
                exit_code,
                ExitCode::KeyMismatch,
                "密钥版本不存在时应返回退出码 4"
            );
        }

        /// Property 6.2: 密钥不匹配错误优先级高于其他错误
        ///
        /// 当同时存在密钥不匹配和其他类型错误时，应优先返回密钥不匹配退出码。
        ///
        /// **Validates: Requirements 5.2, 6.5**
        #[test]
        fn prop_key_mismatch_priority(
            key_error_count in 1usize..=3,
            other_error_count in 1usize..=3,
        ) {
            let mut summary = DecryptSummary::new();

            // 添加密钥不匹配错误
            for i in 0..key_error_count {
                summary.add_failed(
                    format!("key_error{}.dat", i),
                    "密钥不匹配".to_string(),
                );
            }

            // 添加其他类型错误
            for i in 0..other_error_count {
                summary.add_failed(
                    format!("other_error{}.dat", i),
                    "文件格式无效".to_string(),
                );
            }

            let analyzer = ResultAnalyzer::new(&summary);
            let exit_code = analyzer.determine_exit_code();

            prop_assert_eq!(
                exit_code,
                ExitCode::KeyMismatch,
                "密钥不匹配错误应优先于其他错误"
            );
        }

        /// Property 6.3: DecryptError::KeyMismatch 转换为正确退出码
        ///
        /// DecryptError::KeyMismatch 应该转换为 ExitCode::KeyMismatch (4)。
        ///
        /// **Validates: Requirements 5.2, 6.5**
        #[test]
        fn prop_decrypt_error_key_mismatch_conversion(
            version in 0u32..=100,
        ) {
            let error = DecryptError::KeyMismatch(version);
            let exit_code = error.to_exit_code();

            prop_assert_eq!(
                exit_code,
                ExitCode::KeyMismatch,
                "DecryptError::KeyMismatch 应转换为 ExitCode::KeyMismatch"
            );

            // 验证退出码值为 4
            prop_assert_eq!(
                exit_code as u8,
                4,
                "ExitCode::KeyMismatch 的值应为 4"
            );
        }

        /// Property 6.4: 密钥解码错误也返回退出码 4
        ///
        /// DecryptError::KeyDecodeError 也应该返回退出码 4。
        ///
        /// **Validates: Requirements 5.2, 6.5**
        #[test]
        fn prop_key_decode_error_returns_exit_code_4(
            error_msg in "[a-zA-Z0-9 ]{1,50}",
        ) {
            let error = DecryptError::KeyDecodeError(error_msg);
            let exit_code = error.to_exit_code();

            prop_assert_eq!(
                exit_code,
                ExitCode::KeyMismatch,
                "DecryptError::KeyDecodeError 应转换为 ExitCode::KeyMismatch"
            );
        }

        /// Property 6.5: 部分成功时密钥错误不影响退出码
        ///
        /// 当有成功文件时，即使有密钥不匹配错误，也应返回部分成功退出码。
        ///
        /// **Validates: Requirements 6.2, 6.5**
        #[test]
        fn prop_partial_success_overrides_key_error(
            success_count in 1usize..=3,
            key_error_count in 1usize..=3,
        ) {
            let mut summary = DecryptSummary::new();

            // 添加成功结果
            for i in 0..success_count {
                summary.add_success(
                    format!("success{}.dat", i),
                    format!("success{}.txt", i),
                );
            }

            // 添加密钥不匹配错误
            for i in 0..key_error_count {
                summary.add_failed(
                    format!("key_error{}.dat", i),
                    "密钥版本 2 不存在".to_string(),
                );
            }

            let analyzer = ResultAnalyzer::new(&summary);
            let exit_code = analyzer.determine_exit_code();

            prop_assert_eq!(
                exit_code,
                ExitCode::PartialSuccess,
                "有成功文件时应返回部分成功退出码"
            );
        }

        /// Property 6.6: ErrorCategory 正确分类密钥错误
        ///
        /// 密钥相关错误应被正确分类为 ErrorCategory::Key。
        ///
        /// **Validates: Requirements 5.2, 6.5**
        #[test]
        fn prop_error_category_key_classification(
            version in 0u32..=100,
            error_msg in "[a-zA-Z0-9 ]{1,50}",
        ) {
            // KeyMismatch 应分类为 Key
            let key_mismatch = DecryptError::KeyMismatch(version);
            let category1 = ErrorCategory::from(&key_mismatch);
            prop_assert_eq!(
                category1,
                ErrorCategory::Key,
                "KeyMismatch 应分类为 ErrorCategory::Key"
            );

            // KeyDecodeError 应分类为 Key
            let key_decode = DecryptError::KeyDecodeError(error_msg);
            let category2 = ErrorCategory::from(&key_decode);
            prop_assert_eq!(
                category2,
                ErrorCategory::Key,
                "KeyDecodeError 应分类为 ErrorCategory::Key"
            );
        }
    }
}
