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

const AMBIGUOUS_CHARS: &[char] = &['0', 'O', 'o', '1', 'l', 'I', '|', '@','5', 'S', '8', 'B', '2', 'Z', '6', 'G', '9', 'g', 'q', 'u', 'v', 'm', '(',')', '[', ']', '{', '}'];
const LOWER_CASE_CHARS: &[char] = &['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z'];
const UPPER_CASE_CHARS: &[char] = &['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z'];
const DIGITS_CHARS: &[char] = &['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'];
const SYMBOL_CHARS: &[char] =&['!', '.', ',', ':', ';', '~', '@', '#', '$', '%', '^', '&', '*', '+', '-', '=', '_', '(',')', '[', ']', '{', '}'];

pub fn build_character_pool(
    config: &PasswordGeneratorConfig, 
) ->Result<Vec<char>, PasswordGeneratorError> {
    let mut pool: Vec<char> = Vec::new();
    if config.include_lowercase {
        pool.extend_from_slice(LOWER_CASE_CHARS);
    }
    if config.include_uppercase {
        pool.extend_from_slice(UPPER_CASE_CHARS);
    }
    if config.include_digits {
        pool.extend_from_slice(DIGITS_CHARS);
    }
    if config.include_symbols {
        pool.extend_from_slice(SYMBOL_CHARS);
    }
    if pool.is_empty(){
        return Err(PasswordGeneratorError::NoCharacterSetSelected);
    }
    if config.exclude_ambiguous {
        pool.retain(|c| !AMBIGUOUS_CHARS.contains(c));
    }
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] 
    fn character_pool_lower_case() {
        let config = PasswordGeneratorConfig {
            include_lowercase:true,
            length: 20,
            include_uppercase: false,
            include_digits: false,
            include_symbols: false,
            exclude_ambiguous: false,
        };
        let result = build_character_pool(&config);
        assert_eq!(result,Ok(LOWER_CASE_CHARS.to_vec()));
    }

    #[test]

    fn character_pool_upper_case() {
        let config = PasswordGeneratorConfig {
            include_lowercase:false,
            length: 20,
            include_uppercase: true,
            include_digits: false,
            include_symbols: false,
            exclude_ambiguous: false,
        };
        let result = build_character_pool(&config);
        assert_eq!(result,Ok(UPPER_CASE_CHARS.to_vec()));
    }

    #[test]
        fn character_pool_symbol() {
        let config = PasswordGeneratorConfig {
            include_lowercase:false,
            length: 20,
            include_uppercase: false,
            include_digits: false,
            include_symbols: true,
            exclude_ambiguous: false,
        };
        let result = build_character_pool(&config);
        assert_eq!(result,Ok(SYMBOL_CHARS.to_vec()));
    }

    #[test]
        fn character_pool_digits() {
        let config = PasswordGeneratorConfig {
            include_lowercase:false,
            length: 20,
            include_uppercase: false,
            include_digits: true,
            include_symbols: false,
            exclude_ambiguous: false,
        };
        let result = build_character_pool(&config);
        assert_eq!(result,Ok(DIGITS_CHARS.to_vec()));
    }

        #[test]
        fn character_pool_ambiguous() {
        let config = PasswordGeneratorConfig {
            include_lowercase:true,
            length: 20,
            include_uppercase: true,
            include_digits: true,
            include_symbols: true,
            exclude_ambiguous: true,
        };
        let result = build_character_pool(&config);

        let pool = result.expect("expected a successful pool");

        for ambiguous_char in AMBIGUOUS_CHARS {
        assert!(
            !pool.contains(ambiguous_char),
            "pool should not contain ambiguous character: {ambiguous_char}"
        );
    }
}

        #[test]
    fn character_pool_all() {
        let config = PasswordGeneratorConfig {
            include_lowercase:true,
            length: 20,
            include_uppercase: true,
            include_digits: true,
            include_symbols: true,
            exclude_ambiguous: false,
        };
        let result = build_character_pool(&config);
        assert_eq!(result,Ok([LOWER_CASE_CHARS,UPPER_CASE_CHARS,DIGITS_CHARS,SYMBOL_CHARS].map(|c|c.to_vec()).into_iter().flatten().collect()));
    }

    #[test]
fn character_pool_no_classes_selected() {
    let config = PasswordGeneratorConfig {
        include_lowercase: false,
        length: 20,
        include_uppercase: false,
        include_digits: false,
        include_symbols: false,
        exclude_ambiguous: true,
    };
    let result = build_character_pool(&config);

    assert_eq!(result, Err(PasswordGeneratorError::NoCharacterSetSelected));
}
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
