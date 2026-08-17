use std::fmt;

#[derive(Debug, Clone)]
pub struct PasswordGeneratorConfig {
    pub length: usize,
    pub include_lowercase: bool,
    pub include_uppercase: bool,
    pub include_digits: bool,
    pub include_symbols: bool,
    pub exclude_ambiguous: bool,
}

impl Default for PasswordGeneratorConfig {
    fn default() -> Self {
        PasswordGeneratorConfig {
            length: 20,
            include_lowercase: true,
            include_uppercase: true,
            include_digits: true,
            include_symbols: true,
            exclude_ambiguous: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PasswordGeneratorError {
    /// Every `include_*` flag in the config was false — nothing to draw characters from.
    NoCharacterSetSelected,
    /// `length` was below the minimum we're willing to generate.
    LengthTooShort { minimum: usize },
}

impl fmt::Display for PasswordGeneratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PasswordGeneratorError::NoCharacterSetSelected => {
                write!(f, "no character set selected. Have at least one character class enabled")
            }
            PasswordGeneratorError::LengthTooShort { minimum } => {
                write!(f, "length too short: minimum is {minimum}")
            }
        }
    }
}

impl std::error::Error for PasswordGeneratorError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_uses_sensible_values() {
        let config = PasswordGeneratorConfig::default();
        assert_eq!(config.length, 20);
        assert!(config.include_lowercase);
        assert!(config.include_uppercase);
        assert!(config.include_digits);
        assert!(config.include_symbols);
        assert!(config.exclude_ambiguous);
    }
}
