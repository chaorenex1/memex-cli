use super::model::Config;

pub fn load_default() -> Config {
    Config {
        profile: "default".to_string(),
    }
}
