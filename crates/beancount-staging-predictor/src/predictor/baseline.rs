use super::Predictor;
use crate::{PredictionInput, TrainingExample};
use beancount_parser::Account;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct PayeeFrequencyPredictor {
    payee_accounts: HashMap<String, HashMap<String, usize>>,
    source_fallback: HashMap<String, String>,
}

impl Predictor for PayeeFrequencyPredictor {
    fn train(examples: &[TrainingExample]) -> Self {
        let mut payee_accounts: HashMap<String, HashMap<String, usize>> = HashMap::new();
        let mut source_accounts: HashMap<String, HashMap<String, usize>> = HashMap::new();

        for example in examples {
            let target = example.target_account.to_string();
            let source = example.source_account.to_string();

            if let Some(payee) = &example.payee {
                payee_accounts
                    .entry(payee.clone())
                    .or_default()
                    .entry(target.clone())
                    .and_modify(|c| *c += 1)
                    .or_insert(1);
            }

            source_accounts
                .entry(source)
                .or_default()
                .entry(target)
                .and_modify(|c| *c += 1)
                .or_insert(1);
        }

        let source_fallback = source_accounts
            .into_iter()
            .filter_map(|(source, targets)| {
                let most_common = targets
                    .into_iter()
                    .max_by_key(|(_, count)| *count)
                    .map(|(account, _)| account)?;
                Some((source, most_common))
            })
            .collect();

        Self {
            payee_accounts,
            source_fallback,
        }
    }

    fn predict(&self, input: &PredictionInput) -> Option<Account> {
        if let Some(payee) = &input.payee
            && let Some(accounts) = self.payee_accounts.get(payee)
        {
            let most_common = accounts
                .iter()
                .max_by_key(|(_, count)| *count)
                .map(|(account, _)| account)?;

            return most_common.parse().ok();
        }

        let source = input.source_account.to_string();
        self.source_fallback
            .get(&source)
            .and_then(|acc| acc.parse().ok())
    }

    fn name(&self) -> &'static str {
        "PayeeFrequency"
    }
}

impl PayeeFrequencyPredictor {
    pub fn stats(&self) -> PredictorStats {
        PredictorStats {
            unique_payees: self.payee_accounts.len(),
            unique_sources: self.source_fallback.len(),
        }
    }
}

#[derive(Debug)]
pub struct PredictorStats {
    pub unique_payees: usize,
    pub unique_sources: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payee_frequency_prediction() {
        let examples = vec![
            TrainingExample {
                source_account: "Assets:Checking".parse().unwrap(),
                payee: Some("REWE".into()),
                narration: "Groceries".into(),
                target_account: "Expenses:Groceries".parse().unwrap(),
            },
            TrainingExample {
                source_account: "Assets:Checking".parse().unwrap(),
                payee: Some("REWE".into()),
                narration: "More groceries".into(),
                target_account: "Expenses:Groceries".parse().unwrap(),
            },
            TrainingExample {
                source_account: "Assets:Checking".parse().unwrap(),
                payee: Some("REWE".into()),
                narration: "Household items".into(),
                target_account: "Expenses:Household".parse().unwrap(),
            },
        ];

        let predictor = PayeeFrequencyPredictor::train(&examples);

        let input = PredictionInput {
            source_account: "Assets:Checking".parse().unwrap(),
            payee: Some("REWE".into()),
            narration: "New purchase".into(),
        };

        let prediction = predictor.predict(&input);
        assert_eq!(
            prediction.map(|a| a.to_string()),
            Some("Expenses:Groceries".into()),
            "Should predict most frequent account (Groceries appears 2x vs Household 1x)"
        );
    }

    #[test]
    fn test_source_fallback_when_payee_unknown() {
        let examples = vec![
            TrainingExample {
                source_account: "Liabilities:CreditCard".parse().unwrap(),
                payee: Some("Various1".into()),
                narration: "Purchase".into(),
                target_account: "Expenses:Shopping".parse().unwrap(),
            },
            TrainingExample {
                source_account: "Liabilities:CreditCard".parse().unwrap(),
                payee: Some("Various2".into()),
                narration: "Purchase".into(),
                target_account: "Expenses:Shopping".parse().unwrap(),
            },
        ];

        let predictor = PayeeFrequencyPredictor::train(&examples);

        let input = PredictionInput {
            source_account: "Liabilities:CreditCard".parse().unwrap(),
            payee: Some("UnknownPayee".into()),
            narration: "New purchase".into(),
        };

        let prediction = predictor.predict(&input);
        assert_eq!(
            prediction.map(|a| a.to_string()),
            Some("Expenses:Shopping".into()),
            "Should fall back to most common target for source account"
        );
    }

    #[test]
    fn test_no_prediction_when_unknown() {
        let examples = vec![TrainingExample {
            source_account: "Assets:Checking".parse().unwrap(),
            payee: Some("KnownPayee".into()),
            narration: "Purchase".into(),
            target_account: "Expenses:Test".parse().unwrap(),
        }];

        let predictor = PayeeFrequencyPredictor::train(&examples);

        let input = PredictionInput {
            source_account: "Liabilities:Unknown".parse().unwrap(),
            payee: Some("UnknownPayee".into()),
            narration: "New purchase".into(),
        };

        let prediction = predictor.predict(&input);
        assert_eq!(
            prediction, None,
            "Should return None when both payee and source are unknown"
        );
    }
}
