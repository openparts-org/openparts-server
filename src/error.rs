use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

pub enum ApiError {
    NotFound(String),
    /// openparts-validator diagnostics blocked the request.
    ValidationFailed(Vec<openparts_validator::Diagnostic>),
    /// Effective Model construction or geometry/artifact generation
    /// failed (e.g. unknown revision, missing dimension -- Testing and
    /// Quality Specification section 7: stop rather than guess).
    GenerationFailed(String),
}

#[derive(Serialize)]
struct DiagnosticBody {
    rule_id: &'static str,
    severity: String,
    entity: String,
    field_path: Option<String>,
    message: String,
}

impl From<&openparts_validator::Diagnostic> for DiagnosticBody {
    fn from(d: &openparts_validator::Diagnostic) -> Self {
        DiagnosticBody {
            rule_id: d.rule_id,
            severity: format!("{:?}", d.severity),
            entity: d.entity.clone(),
            field_path: d.field_path.clone(),
            message: d.message.clone(),
        }
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<DiagnosticBody>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            ApiError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                ErrorBody {
                    error: msg,
                    diagnostics: vec![],
                },
            ),
            ApiError::ValidationFailed(diags) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                ErrorBody {
                    error: "validation failed".to_string(),
                    diagnostics: diags.iter().map(DiagnosticBody::from).collect(),
                },
            ),
            ApiError::GenerationFailed(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                ErrorBody {
                    error: msg,
                    diagnostics: vec![],
                },
            ),
        };
        (status, Json(body)).into_response()
    }
}
