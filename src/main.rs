#[tokio::main]
async fn main() -> Result<(), tedxachievers::errors::AppError> {
    tedxachievers::run().await
}
