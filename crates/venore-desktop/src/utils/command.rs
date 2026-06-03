//! Command utilities for Tauri IPC
//!
//! Provides standardized command result wrapper for consistent error handling

use serde::Serialize;
use venore_core::error::{VenoreError, ErrorResponse};

/// Standard command result for Tauri IPC
///
/// This enum provides a consistent response format for all Tauri commands.
/// It serializes to JSON with a "success" field that's either "true" or "false".
///
/// # Examples
/// ```
/// use venore_desktop_lib::utils::CommandResult;
/// use venore_core::error::VenoreError;
///
/// // From Result<T, VenoreError>
/// let result: Result<String, VenoreError> = Ok("success".to_string());
/// let cmd_result: CommandResult<String> = result.into();
///
/// // Direct construction
/// let cmd_result = CommandResult::ok("success".to_string());
/// ```
#[derive(Serialize)]
#[serde(tag = "success", rename_all = "camelCase")]
pub enum CommandResult<T: Serialize> {
    #[serde(rename = "true")]
    Ok { data: T },

    #[serde(rename = "false")]
    Err { error: ErrorResponse },
}

/// Auto-conversion from Result<T, VenoreError>
///
/// This allows you to use `?` operator in Tauri commands and automatically
/// convert the result to CommandResult.
///
/// # Examples
/// ```
/// use venore_desktop_lib::utils::CommandResult;
/// use venore_core::error::{VenoreError, Result};
///
/// fn process() -> Result<String> {
///     Ok("processed".to_string())
/// }
///
/// // In a Tauri command
/// // #[tauri::command]
/// // async fn my_command() -> CommandResult<String> {
/// //     let result = process()?;
/// //     Ok(result).into()
/// // }
/// ```
impl<T: Serialize> From<Result<T, VenoreError>> for CommandResult<T> {
    fn from(result: Result<T, VenoreError>) -> Self {
        match result {
            Ok(data) => CommandResult::Ok { data },
            Err(err) => CommandResult::Err {
                error: err.into(),
            },
        }
    }
}

/// Type alias for async commands that have reference inputs (e.g. tauri::State, Window).
///
/// Tauri v2 requires async commands with reference parameters to return `Result<T, E>`.
/// This wraps CommandResult in a Result that always succeeds (Ok), preserving the
/// same serialized JSON format the frontend expects.
///
/// The `Err(())` case is never used — all errors are encoded inside `CommandResult::Err`.
pub type StateCommandResult<T> = std::result::Result<CommandResult<T>, ()>;

/// Convert CommandResult to StateCommandResult (always Ok)
impl<T: Serialize> From<CommandResult<T>> for StateCommandResult<T> {
    fn from(result: CommandResult<T>) -> Self {
        Ok(result)
    }
}

/// Helper methods for constructing CommandResult
impl<T: Serialize> CommandResult<T> {
    /// Create a successful result
    pub fn ok(data: T) -> Self {
        CommandResult::Ok { data }
    }

    /// Create an error result
    pub fn err(error: VenoreError) -> Self {
        CommandResult::Err {
            error: error.into(),
        }
    }

    /// Wrap in StateCommandResult (for async commands with reference inputs)
    // Tauri v2 forces async commands with reference inputs to return Result<T, E>;
    // the Err half is unreachable — real errors live inside CommandResult::Err.
    #[allow(clippy::result_unit_err)]
    pub fn into_state(self) -> StateCommandResult<T> {
        Ok(self)
    }
}

/// Extension trait for Result<T, VenoreError> to convert to StateCommandResult
pub trait IntoStateCommandResult<T: Serialize> {
    // Tauri v2 forces async commands with reference inputs to return Result<T, E>;
    // the Err half is unreachable — real errors live inside CommandResult::Err.
    #[allow(clippy::result_unit_err)]
    fn into_state(self) -> StateCommandResult<T>;
}

impl<T: Serialize> IntoStateCommandResult<T> for Result<T, VenoreError> {
    fn into_state(self) -> StateCommandResult<T> {
        let cmd_result: CommandResult<T> = self.into();
        Ok(cmd_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use venore_core::error::VenoreError;

    #[test]
    fn test_command_result_ok() {
        let result: CommandResult<String> = Ok("success".to_string()).into();

        let json = serde_json::to_value(result).unwrap();
        assert_eq!(json["success"], "true");
        assert_eq!(json["data"], "success");
    }

    #[test]
    fn test_command_result_err() {
        let error = VenoreError::FileNotFound("test.txt".to_string());
        let result: CommandResult<String> = Err(error).into();

        let json = serde_json::to_value(result).unwrap();
        assert_eq!(json["success"], "false");
        assert!(json["error"]["code"].as_str().is_some());
    }

    #[test]
    fn test_command_result_ok_helper() {
        let result = CommandResult::ok("data".to_string());

        let json = serde_json::to_value(result).unwrap();
        assert_eq!(json["success"], "true");
        assert_eq!(json["data"], "data");
    }

    #[test]
    fn test_command_result_err_helper() {
        let result: CommandResult<String> = CommandResult::err(
            VenoreError::NotFound("item".to_string())
        );

        let json = serde_json::to_value(result).unwrap();
        assert_eq!(json["success"], "false");
        assert!(json["error"]["message"].as_str().unwrap().contains("Not found"));
    }

    #[test]
    fn test_serialization_format() {
        let result: CommandResult<i32> = Ok(42).into();
        let json = serde_json::to_string(&result).unwrap();

        // Should have "success":"true" (as string, not boolean)
        assert!(json.contains("\"success\":\"true\""));
        assert!(json.contains("\"data\":42"));
    }

    #[test]
    fn test_error_serialization_format() {
        let error = VenoreError::Timeout(5000);
        let result: CommandResult<String> = Err(error).into();
        let json = serde_json::to_string(&result).unwrap();

        // Should have "success":"false" (as string, not boolean)
        assert!(json.contains("\"success\":\"false\""));
        assert!(json.contains("\"error\""));
        assert!(json.contains("\"code\""));
        assert!(json.contains("\"message\""));
    }
}
