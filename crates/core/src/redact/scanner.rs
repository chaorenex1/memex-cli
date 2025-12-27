pub fn redact(input: &str) -> String {
    input.replace("secret", "[redacted]")
}
