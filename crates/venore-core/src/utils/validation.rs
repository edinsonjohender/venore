//! Validation utilities for composable validators

/// Validation result
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationResult {
    pub valid: bool,
    pub error: Option<String>,
}

impl ValidationResult {
    pub fn ok() -> Self {
        Self { valid: true, error: None }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            valid: false,
            error: Some(message.into()),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }
}

/// Validate required field (non-empty)
///
/// # Examples
/// ```
/// use venore_core::utils::validate_required;
///
/// assert!(validate_required("value", "field").is_valid());
/// assert!(!validate_required("", "field").is_valid());
/// assert!(!validate_required("   ", "field").is_valid());
/// ```
pub fn validate_required(value: &str, field_name: &str) -> ValidationResult {
    if value.trim().is_empty() {
        ValidationResult::err(format!("{} is required", field_name))
    } else {
        ValidationResult::ok()
    }
}

/// Validate project name (2-100 characters)
///
/// # Examples
/// ```
/// use venore_core::utils::validate_project_name;
///
/// assert!(validate_project_name("my-project").is_valid());
/// assert!(!validate_project_name("a").is_valid());
/// ```
pub fn validate_project_name(name: &str) -> ValidationResult {
    if name.len() < 2 {
        return ValidationResult::err("Project name must be at least 2 characters");
    }
    if name.len() > 100 {
        return ValidationResult::err("Project name must be at most 100 characters");
    }
    ValidationResult::ok()
}

/// Validate project description (min length)
///
/// # Examples
/// ```
/// use venore_core::utils::validate_project_description;
///
/// let long_desc = "This is a long enough description for the project";
/// assert!(validate_project_description(long_desc, Some(20)).is_valid());
/// assert!(!validate_project_description("short", Some(20)).is_valid());
/// ```
pub fn validate_project_description(desc: &str, min_length: Option<usize>) -> ValidationResult {
    let min = min_length.unwrap_or(20);
    if desc.len() < min {
        return ValidationResult::err(format!(
            "Description must be at least {} characters",
            min
        ));
    }
    ValidationResult::ok()
}

/// Validate email (basic regex)
///
/// # Examples
/// ```
/// use venore_core::utils::validate_email;
///
/// assert!(validate_email("user@example.com").is_valid());
/// assert!(!validate_email("invalid-email").is_valid());
/// ```
pub fn validate_email(email: &str) -> ValidationResult {
    let re = regex::Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").unwrap();
    if re.is_match(email) {
        ValidationResult::ok()
    } else {
        ValidationResult::err("Invalid email format")
    }
}

/// Validate URL
///
/// # Examples
/// ```
/// use venore_core::utils::validate_url;
///
/// assert!(validate_url("https://example.com").is_valid());
/// assert!(!validate_url("not-a-url").is_valid());
/// ```
pub fn validate_url(url: &str) -> ValidationResult {
    match url::Url::parse(url) {
        Ok(_) => ValidationResult::ok(),
        Err(_) => ValidationResult::err("Invalid URL format"),
    }
}

/// Validate range (number between min and max)
///
/// # Examples
/// ```
/// use venore_core::utils::validate_range;
///
/// assert!(validate_range(5, 1, 10, "value").is_valid());
/// assert!(!validate_range(15, 1, 10, "value").is_valid());
/// ```
pub fn validate_range(
    num: i64,
    min: i64,
    max: i64,
    field_name: &str,
) -> ValidationResult {
    if num < min || num > max {
        ValidationResult::err(format!(
            "{} must be between {} and {}",
            field_name, min, max
        ))
    } else {
        ValidationResult::ok()
    }
}

/// Validate positive number
///
/// # Examples
/// ```
/// use venore_core::utils::validate_positive;
///
/// assert!(validate_positive(5, "count").is_valid());
/// assert!(!validate_positive(0, "count").is_valid());
/// assert!(!validate_positive(-5, "count").is_valid());
/// ```
pub fn validate_positive(num: i64, field_name: &str) -> ValidationResult {
    if num <= 0 {
        ValidationResult::err(format!("{} must be positive", field_name))
    } else {
        ValidationResult::ok()
    }
}

/// Validate pattern (regex)
///
/// # Examples
/// ```
/// use venore_core::utils::validate_pattern;
///
/// assert!(validate_pattern("abc123", r"^[a-z0-9]+$", "username").is_valid());
/// assert!(!validate_pattern("ABC", r"^[a-z]+$", "lowercase").is_valid());
/// ```
pub fn validate_pattern(
    value: &str,
    pattern: &str,
    field_name: &str,
) -> ValidationResult {
    let re = regex::Regex::new(pattern).unwrap();
    if re.is_match(value) {
        ValidationResult::ok()
    } else {
        ValidationResult::err(format!("{} does not match required pattern", field_name))
    }
}

/// Validate file extension
///
/// # Examples
/// ```
/// use venore_core::utils::validate_file_extension;
///
/// assert!(validate_file_extension("file.rs", &["rs", "toml"]).is_valid());
/// assert!(!validate_file_extension("file.txt", &["rs", "toml"]).is_valid());
/// ```
pub fn validate_file_extension(filename: &str, allowed: &[&str]) -> ValidationResult {
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str());

    match ext {
        Some(e) if allowed.contains(&e) => ValidationResult::ok(),
        _ => ValidationResult::err(format!(
            "File extension must be one of: {}",
            allowed.join(", ")
        )),
    }
}

/// Validate all (returns first error or ok)
///
/// # Examples
/// ```
/// use venore_core::utils::{validate_all, validate_required, ValidationResult};
///
/// let results = vec![
///     validate_required("value", "field1"),
///     validate_required("value2", "field2"),
/// ];
/// assert!(validate_all(results).is_valid());
/// ```
pub fn validate_all(results: Vec<ValidationResult>) -> ValidationResult {
    for result in results {
        if !result.valid {
            return result;
        }
    }
    ValidationResult::ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_required() {
        assert!(validate_required("value", "field").is_valid());
        assert!(!validate_required("", "field").is_valid());
        assert!(!validate_required("   ", "field").is_valid());

        let result = validate_required("", "name");
        assert_eq!(result.error, Some("name is required".to_string()));
    }

    #[test]
    fn test_validate_project_name() {
        assert!(validate_project_name("my-project").is_valid());
        assert!(validate_project_name("ab").is_valid());
        assert!(!validate_project_name("a").is_valid());
        assert!(!validate_project_name(&"a".repeat(101)).is_valid());
    }

    #[test]
    fn test_validate_project_description() {
        let long_desc = "This is a long enough description";
        assert!(validate_project_description(long_desc, Some(20)).is_valid());
        assert!(!validate_project_description("short", Some(20)).is_valid());

        // Default min length is 20
        assert!(validate_project_description(long_desc, None).is_valid());
    }

    #[test]
    fn test_validate_email() {
        assert!(validate_email("user@example.com").is_valid());
        assert!(validate_email("test.user@domain.co.uk").is_valid());
        assert!(!validate_email("invalid-email").is_valid());
        assert!(!validate_email("@example.com").is_valid());
        assert!(!validate_email("user@").is_valid());
    }

    #[test]
    fn test_validate_url() {
        assert!(validate_url("https://example.com").is_valid());
        assert!(validate_url("http://localhost:3000").is_valid());
        assert!(!validate_url("not-a-url").is_valid());
        assert!(!validate_url("://invalid").is_valid());
    }

    #[test]
    fn test_validate_range() {
        assert!(validate_range(5, 1, 10, "value").is_valid());
        assert!(validate_range(1, 1, 10, "value").is_valid());
        assert!(validate_range(10, 1, 10, "value").is_valid());
        assert!(!validate_range(0, 1, 10, "value").is_valid());
        assert!(!validate_range(11, 1, 10, "value").is_valid());
    }

    #[test]
    fn test_validate_positive() {
        assert!(validate_positive(1, "count").is_valid());
        assert!(validate_positive(100, "count").is_valid());
        assert!(!validate_positive(0, "count").is_valid());
        assert!(!validate_positive(-5, "count").is_valid());
    }

    #[test]
    fn test_validate_pattern() {
        assert!(validate_pattern("abc123", r"^[a-z0-9]+$", "username").is_valid());
        assert!(validate_pattern("hello", r"^[a-z]+$", "lowercase").is_valid());
        assert!(!validate_pattern("ABC", r"^[a-z]+$", "lowercase").is_valid());
    }

    #[test]
    fn test_validate_file_extension() {
        assert!(validate_file_extension("file.rs", &["rs", "toml"]).is_valid());
        assert!(validate_file_extension("config.toml", &["rs", "toml"]).is_valid());
        assert!(!validate_file_extension("file.txt", &["rs", "toml"]).is_valid());
        assert!(!validate_file_extension("noextension", &["rs", "toml"]).is_valid());
    }

    #[test]
    fn test_validate_all() {
        let results = vec![
            ValidationResult::ok(),
            ValidationResult::ok(),
        ];
        assert!(validate_all(results).is_valid());

        let results = vec![
            ValidationResult::ok(),
            ValidationResult::err("Error 1"),
            ValidationResult::err("Error 2"),
        ];
        let result = validate_all(results);
        assert!(!result.is_valid());
        assert_eq!(result.error, Some("Error 1".to_string()));
    }

    #[test]
    fn test_validation_result_eq() {
        assert_eq!(ValidationResult::ok(), ValidationResult::ok());
        assert_eq!(
            ValidationResult::err("error"),
            ValidationResult::err("error")
        );
        assert_ne!(ValidationResult::ok(), ValidationResult::err("error"));
    }
}
