//! 密码管理模块
//!
//! 提供密码哈希、验证和强度检查功能，使用 Argon2id 算法。

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

use super::error::WebAuthError;

/// 最小密码长度
pub const MIN_PASSWORD_LENGTH: usize = 8;

/// 密码管理器
pub struct PasswordManager;

impl PasswordManager {
    /// 使用 Argon2id 算法哈希密码
    ///
    /// # Arguments
    /// * `password` - 明文密码
    ///
    /// # Returns
    /// * `Ok(String)` - PHC 格式的密码哈希字符串
    /// * `Err(WebAuthError)` - 哈希失败
    ///
    /// # Example
    /// ```ignore
    /// let hash = PasswordManager::hash_password("my_secure_password")?;
    /// ```
    pub fn hash_password(password: &str) -> Result<String, WebAuthError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        argon2
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|e| WebAuthError::HashError(e.to_string()))
    }

    /// 验证密码是否与哈希匹配
    ///
    /// # Arguments
    /// * `password` - 明文密码
    /// * `hash` - PHC 格式的密码哈希字符串
    ///
    /// # Returns
    /// * `Ok(true)` - 密码匹配
    /// * `Ok(false)` - 密码不匹配
    /// * `Err(WebAuthError)` - 验证过程出错
    pub fn verify_password(password: &str, hash: &str) -> Result<bool, WebAuthError> {
        let parsed_hash =
            PasswordHash::new(hash).map_err(|e| WebAuthError::HashError(e.to_string()))?;

        let argon2 = Argon2::default();

        match argon2.verify_password(password.as_bytes(), &parsed_hash) {
            Ok(()) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(e) => Err(WebAuthError::HashError(e.to_string())),
        }
    }

    /// 验证密码强度
    ///
    /// 当前仅检查最小长度（8 字符），可扩展更多规则。
    ///
    /// # Arguments
    /// * `password` - 待验证的密码
    ///
    /// # Returns
    /// * `Ok(())` - 密码符合要求
    /// * `Err(WebAuthError::InvalidPassword)` - 密码不符合要求
    pub fn validate_strength(password: &str) -> Result<(), WebAuthError> {
        if password.len() < MIN_PASSWORD_LENGTH {
            return Err(WebAuthError::InvalidPassword(format!(
                "密码长度至少需要 {} 个字符",
                MIN_PASSWORD_LENGTH
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_password() {
        let password = "test_password_123";
        let hash = PasswordManager::hash_password(password).unwrap();

        // 验证哈希格式（PHC 格式以 $argon2 开头）
        assert!(hash.starts_with("$argon2"));
    }

    #[test]
    fn test_verify_password_correct() {
        let password = "correct_password";
        let hash = PasswordManager::hash_password(password).unwrap();

        assert!(PasswordManager::verify_password(password, &hash).unwrap());
    }

    #[test]
    fn test_verify_password_incorrect() {
        let password = "correct_password";
        let hash = PasswordManager::hash_password(password).unwrap();

        assert!(!PasswordManager::verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn test_validate_strength_valid() {
        assert!(PasswordManager::validate_strength("12345678").is_ok());
        assert!(PasswordManager::validate_strength("a_very_long_password").is_ok());
    }

    #[test]
    fn test_validate_strength_too_short() {
        assert!(PasswordManager::validate_strength("1234567").is_err());
        assert!(PasswordManager::validate_strength("").is_err());
        assert!(PasswordManager::validate_strength("short").is_err());
    }

    #[test]
    fn test_different_passwords_different_hashes() {
        let hash1 = PasswordManager::hash_password("password1").unwrap();
        let hash2 = PasswordManager::hash_password("password1").unwrap();

        // 由于盐值不同，相同密码的哈希也不同
        assert_ne!(hash1, hash2);

        // 但两个哈希都能验证原密码
        assert!(PasswordManager::verify_password("password1", &hash1).unwrap());
        assert!(PasswordManager::verify_password("password1", &hash2).unwrap());
    }
}
