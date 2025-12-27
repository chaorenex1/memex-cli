use super::templates;

pub struct ContextBuilder;

impl ContextBuilder {
    pub fn build() -> String {
        templates::default_template().to_string()
    }
}
