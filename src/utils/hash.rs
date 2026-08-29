use crate::errors::AppError;
pub fn hash_password(password: &str) -> Result<String, AppError> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST)
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))
}
pub fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
    bcrypt::verify(password, hash).map_err(|error| AppError::Internal(anyhow::anyhow!(error)))
}
