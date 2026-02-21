use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use mindbox_common::MindboxError;
use serde::Serialize;

#[derive(Debug)]
pub struct ApiError(pub MindboxError);

pub type ApiResult<T> = std::result::Result<T, ApiError>;

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.0 {
            MindboxError::TaskNotFound(_) => StatusCode::NOT_FOUND,
            MindboxError::TaskLockBusy => StatusCode::CONFLICT,
            MindboxError::InvalidStateTransition { .. } | MindboxError::Config(_) => {
                StatusCode::BAD_REQUEST
            }
            MindboxError::Cancelled(_) => StatusCode::CONFLICT,
            MindboxError::KernelError(_)
            | MindboxError::IO(_)
            | MindboxError::Yaml(_)
            | MindboxError::Json(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = Json(ErrorBody {
            error: self.0.to_string(),
        });
        (status, body).into_response()
    }
}

impl From<MindboxError> for ApiError {
    fn from(value: MindboxError) -> Self {
        Self(value)
    }
}
