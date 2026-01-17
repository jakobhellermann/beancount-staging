use std::sync::LazyLock;

// Pattern definitions for testing
const PATTERN_DATE_ISO: &str = r"\b\d{4}[-/]\d{1,2}[-/]\d{1,2}\b";
const PATTERN_DATE_DMY: &str = r"\b\d{1,2}[./]\d{1,2}[./]\d{2,4}\b";
const PATTERN_TIME: &str = r"\b\d{1,2}:\d{2}(:\d{2})?\b";
const PATTERN_LONG_NUMBERS: &str = r"\b\d{4,}\b";
const PATTERN_REF_ID: &str = r"\b(eref|mref|cred|ref)\s*:?\s*[a-z0-9\-/]+";
const PATTERN_NR_ID: &str = r"\bnr\.?\s*\d+";
const PATTERN_IBAN: &str = r"\b(iban|bic)\s*:\s*[a-z0-9]+";
const PATTERN_TRANSACTION_CODES: &str = r"/+\d*/*\d*/*\s*(ectl|cicc|fpin|npin)";
const PATTERN_SLASH_NUMBERS: &str = r"/+\d+/+";
const PATTERN_PP_REF: &str = r"\bpp\.\d+\.pp\b";

// Compiled regexes (lazy-initialized once at program startup)
static NOISE_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        regex::Regex::new(PATTERN_DATE_ISO).unwrap(),
        regex::Regex::new(PATTERN_DATE_DMY).unwrap(),
        regex::Regex::new(PATTERN_TIME).unwrap(),
        regex::Regex::new(PATTERN_IBAN).unwrap(),
        regex::Regex::new(PATTERN_TRANSACTION_CODES).unwrap(),
        regex::Regex::new(PATTERN_PP_REF).unwrap(),
        regex::Regex::new(PATTERN_SLASH_NUMBERS).unwrap(),
        regex::Regex::new(PATTERN_LONG_NUMBERS).unwrap(),
        regex::Regex::new(PATTERN_REF_ID).unwrap(),
        regex::Regex::new(PATTERN_NR_ID).unwrap(),
    ]
});

/// Trait for text preprocessing strategies
pub trait Preprocessor {
    fn preprocess(&self, text: &str) -> String;
}

/// No preprocessing - just lowercase
#[derive(Default, Debug, Clone, Copy)]
pub struct Raw;
impl Preprocessor for Raw {
    fn preprocess(&self, text: &str) -> String {
        text.to_lowercase()
    }
}

/// Remove all non-alphabetic characters
#[derive(Default, Debug, Clone, Copy)]
pub struct Alpha;
impl Preprocessor for Alpha {
    fn preprocess(&self, text: &str) -> String {
        let text = text.to_lowercase();

        // Keep only alphabetic chars and spaces
        let text: String = text
            .chars()
            .map(|c| {
                if c.is_alphabetic() || c.is_whitespace() {
                    c
                } else {
                    ' '
                }
            })
            .collect();

        // Normalize whitespace
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

/// Smart preprocessing - remove dates, IDs, IBANs, etc.
#[derive(Default, Debug, Clone, Copy)]
pub struct Smart;
impl Preprocessor for Smart {
    fn preprocess(&self, text: &str) -> String {
        let mut text = text.to_lowercase();

        // Apply all noise removal patterns
        for pattern in NOISE_PATTERNS.iter() {
            text = pattern.replace_all(&text, "").to_string();
        }

        // Normalize whitespace
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

/// Legacy function - use Smart preprocessor instead
pub fn preprocess_text(text: &str) -> String {
    Smart.preprocess(text)
}

/// Legacy function - use Alpha preprocessor instead
pub fn preprocess_alpha_only(text: &str) -> String {
    Alpha.preprocess(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_pattern_removes(pattern_str: &str, input: &str, expected_output: &str) {
        let re = regex::Regex::new(pattern_str).unwrap();
        let lowercased = input.to_lowercase();
        let result = re.replace_all(&lowercased, "");
        let normalized = result.split_whitespace().collect::<Vec<_>>().join(" ");
        assert_eq!(
            normalized, expected_output,
            "Pattern '{}' applied to '{}' should produce '{}', got '{}'",
            pattern_str, input, expected_output, normalized
        );
    }

    #[test]
    fn test_pattern_date_iso() {
        test_pattern_removes(PATTERN_DATE_ISO, "Purchase on 2026-01-15", "purchase on");
    }

    #[test]
    fn test_pattern_date_dmy() {
        test_pattern_removes(PATTERN_DATE_DMY, "LIDL 10.01.2026 store", "lidl store");
    }

    #[test]
    fn test_pattern_time() {
        test_pattern_removes(PATTERN_TIME, "Purchase at 14:30:45", "purchase at");
        test_pattern_removes(PATTERN_TIME, "um 20:34 Uhr", "um uhr");
    }

    #[test]
    fn test_pattern_long_numbers() {
        test_pattern_removes(PATTERN_LONG_NUMBERS, "ID 123456 verified", "id verified");
    }

    #[test]
    fn test_pattern_ref_id() {
        test_pattern_removes(PATTERN_REF_ID, "LIDL EREF: LG--100-00172", "lidl");
    }

    #[test]
    fn test_pattern_iban() {
        test_pattern_removes(PATTERN_IBAN, "Transfer IBAN: DE74200411", "transfer");
    }

    #[test]
    fn test_pattern_pp_ref() {
        test_pattern_removes(
            PATTERN_PP_REF,
            "PayPal PP.8571.PP payment",
            "paypal payment",
        );
    }

    #[test]
    fn test_preprocess_removes_dates() {
        assert_eq!(
            preprocess_text("LIDL SAGT DANKE/Essen 10.01.2026 purchase"),
            "lidl sagt danke/essen purchase"
        );
    }

    #[test]
    fn test_preprocess_removes_times() {
        assert_eq!(
            preprocess_text("Purchase at 14:30:45 um 20:34:34"),
            "purchase at um"
        );
    }

    #[test]
    fn test_preprocess_removes_long_numbers() {
        assert_eq!(
            preprocess_text("Transaction 1234 and 567890 completed"),
            "transaction and completed"
        );
    }

    #[test]
    fn test_preprocess_removes_references() {
        assert_eq!(
            preprocess_text("LIDL EREF: LG--100-00172 MREF: 54PJ224ZVTCU8"),
            "lidl"
        );
    }

    #[test]
    fn test_preprocess_normalizes_whitespace() {
        assert_eq!(
            preprocess_text("  Multiple   spaces   here  "),
            "multiple spaces here"
        );
    }

    #[test]
    fn test_preprocess_complex_example() {
        assert_eq!(
            preprocess_text("LIDL SAGT DANKE/Essen 10.01.2026 um 20:34:34 REF 101002/260045"),
            "lidl sagt danke/essen um"
        );
    }

    #[test]
    fn test_preprocess_preserves_short_numbers() {
        assert_eq!(
            preprocess_text("Item 3 costs 25 EUR"),
            "item 3 costs 25 eur"
        );
    }

    #[test]
    fn test_preprocess_removes_iban_bic() {
        assert_eq!(
            preprocess_text("Transfer IBAN: DE74200411110562880500 BIC: COBADEHDXXX"),
            "transfer"
        );
    }

    #[test]
    fn test_preprocess_removes_transaction_codes() {
        assert_eq!(
            preprocess_text("Purchase /042516/ECTL/NPIN //1/1223"),
            "purchase"
        );
    }

    #[test]
    fn test_preprocess_real_transaction() {
        assert_eq!(
            preprocess_text(
                "Kartenzahlung girocard ALDI SAGT DANKE 01 071/Essen-Altenessen/DE 10.01.2026 um 20:34:34 /383141/CICC/FPIN //1/1223"
            ),
            "kartenzahlung girocard aldi sagt danke 01 071/essen-altenessen/de um"
        );
    }
}
