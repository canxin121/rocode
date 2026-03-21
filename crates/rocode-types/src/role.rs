use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, Display, EnumString};

/// Role of a conversation message.
#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Display, EnumString, AsRefStr,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_case_insensitive() {
        assert_eq!("USER".parse::<Role>().ok(), Some(Role::User));
        assert_eq!("Assistant".parse::<Role>().ok(), Some(Role::Assistant));
        assert_eq!("unknown".parse::<Role>().ok(), None);
    }
}
