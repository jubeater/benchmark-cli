pub fn render(accepted: usize, rejected: usize) -> String {
    format!(
        "Eval-schema v2 validation\nAccepted records: {accepted}\nRejected records: {rejected}\n"
    )
}
