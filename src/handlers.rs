use axum::http::StatusCode;

/// Utility function for mapping any error into a `500 Internal Server Error`
pub fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;

    #[derive(Debug)]
    struct TestError(String);

    impl fmt::Display for TestError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl std::error::Error for TestError {}

    #[test]
    fn test_internal_error_returns_500_status() {
        let (status, _) = internal_error(TestError("boom".to_string()));
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_internal_error_returns_error_message_as_body() {
        let (_, body) = internal_error(TestError("something went wrong".to_string()));
        assert_eq!(body, "something went wrong");
    }

    #[test]
    fn test_internal_error_with_empty_message() {
        let (status, body) = internal_error(TestError(String::new()));
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body, "");
    }
}
