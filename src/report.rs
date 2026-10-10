pub fn render(accepted: usize, rejected: usize, superseded: usize) -> String {
    format!(
        "Eval-schema v2 validation\nAccepted records: {accepted}\nRejected records: {rejected}\nSuperseded records: {superseded}\n"
    )
}
