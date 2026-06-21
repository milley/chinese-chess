//! Input validation utilities for request data.

use crate::error::AppError;

/// Maximum length for a UUID string (standard format: 8-4-4-4-12 = 36 chars)
const MAX_UUID_LEN: usize = 36;

/// Maximum length for a JWT token string
const MAX_TOKEN_LEN: usize = 1024;

/// Validate a board position string (format: "a0"-"i9", exactly 2 chars).
///
/// Chinese chess board positions use:
/// - Column: 'a' through 'i' (9 columns)
/// - Row: '0' through '9' (10 rows)
pub fn validate_position_string(s: &str) -> Result<(), AppError> {
    let bytes = s.as_bytes();
    if bytes.len() != 2 {
        return Err(AppError::BadRequest("Position must be 2 characters".into()));
    }
    if !matches!(bytes[0], b'a'..=b'i') {
        return Err(AppError::BadRequest("Column must be a-i".into()));
    }
    if !matches!(bytes[1], b'0'..=b'9') {
        return Err(AppError::BadRequest("Row must be 0-9".into()));
    }
    Ok(())
}

/// Validate a game ID string (should be a UUID, max 36 chars).
pub fn validate_game_id_string(s: &str) -> Result<(), AppError> {
    if s.is_empty() {
        return Err(AppError::BadRequest("Game ID cannot be empty".into()));
    }
    if s.len() > MAX_UUID_LEN {
        return Err(AppError::BadRequest("Game ID too long".into()));
    }
    Ok(())
}

/// Validate a JWT token string (max 1024 chars).
pub fn validate_token_string(s: &str) -> Result<(), AppError> {
    if s.is_empty() {
        return Err(AppError::BadRequest("Token cannot be empty".into()));
    }
    if s.len() > MAX_TOKEN_LEN {
        return Err(AppError::BadRequest("Token too long".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // === validate_position_string tests ===

    #[test]
    fn test_position_valid() {
        assert!(validate_position_string("a0").is_ok());
        assert!(validate_position_string("i9").is_ok());
        assert!(validate_position_string("e5").is_ok());
        assert!(validate_position_string("b7").is_ok());
    }

    #[test]
    fn test_position_wrong_length() {
        assert!(validate_position_string("").is_err());
        assert!(validate_position_string("a").is_err());
        assert!(validate_position_string("a01").is_err());
        assert!(validate_position_string("  ").is_err());
    }

    #[test]
    fn test_position_invalid_column() {
        // Column before 'a'
        assert!(validate_position_string("`0").is_err());
        // Column after 'i'
        assert!(validate_position_string("j0").is_err());
        // Uppercase not allowed
        assert!(validate_position_string("A0").is_err());
        // Digit as column
        assert!(validate_position_string("10").is_err());
    }

    #[test]
    fn test_position_invalid_row() {
        // Row outside 0-9
        assert!(validate_position_string("a:").is_err());
        assert!(validate_position_string("a/").is_err());
        // Letter as row
        assert!(validate_position_string("aa").is_err());
    }

    #[test]
    fn test_position_long_string_rejected() {
        assert!(validate_position_string("a0AAAAAAAAAAAAAAAAAAAAAAAAAAAAA").is_err());
    }

    // === validate_game_id_string tests ===

    #[test]
    fn test_game_id_valid() {
        assert!(validate_game_id_string("123e4567-e89b-12d3-a456-426614174000").is_ok());
        assert!(validate_game_id_string("abc").is_ok());
    }

    #[test]
    fn test_game_id_empty() {
        assert!(validate_game_id_string("").is_err());
    }

    #[test]
    fn test_game_id_too_long() {
        let long_id = "x".repeat(37);
        assert!(validate_game_id_string(&long_id).is_err());
        // Exactly 36 chars is ok (UUID format)
        let uuid_len = "x".repeat(36);
        assert!(validate_game_id_string(&uuid_len).is_ok());
    }

    // === validate_token_string tests ===

    #[test]
    fn test_token_valid() {
        assert!(validate_token_string("abc123").is_ok());
        assert!(validate_token_string("eyJhbGciOiJIUzI1NiJ9.test").is_ok());
    }

    #[test]
    fn test_token_empty() {
        assert!(validate_token_string("").is_err());
    }

    #[test]
    fn test_token_too_long() {
        let long_token = "x".repeat(1025);
        assert!(validate_token_string(&long_token).is_err());
        // Exactly 1024 chars is ok
        let max_token = "x".repeat(1024);
        assert!(validate_token_string(&max_token).is_ok());
    }
}
