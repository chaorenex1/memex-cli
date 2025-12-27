pub mod rules;
pub mod r#trait;

use async_trait::async_trait;
use crate::policy::r#trait::Approver;

pub struct ConsoleApprover;

#[async_trait]
impl Approver for ConsoleApprover {
    fn approve(&self, _prompt: &str) -> anyhow::Result<bool> {
        // In a real CLI this would ask the user. For now, just allow.
        Ok(true)
    }
}
