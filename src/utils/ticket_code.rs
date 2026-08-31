use rand::{distributions::Alphanumeric, Rng};
pub fn generate_ticket_code() -> String {
    let code: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .filter(|character| character.is_ascii_alphanumeric())
        .take(6)
        .map(char::from)
        .map(|character| character.to_ascii_uppercase())
        .collect();
    format!("TEDxACH-{code}")
}
