use super::Predictor;
use crate::{PredictionInput, TrainingExample, features::FeatureExtractor};
use beancount_parser::Account;
use smartcore::linalg::basic::matrix::DenseMatrix;
use smartcore::naive_bayes::multinomial::MultinomialNB;
use std::collections::HashMap;

/// N-gram ML predictor using Multinomial Naive Bayes (smart preprocessing)
pub struct MultinomialNBPredictor {
    classifier: MultinomialNB<u32, u32, DenseMatrix<u32>, Vec<u32>>,
    feature_extractor: FeatureExtractor,
    label_to_account: Vec<String>,
    #[allow(dead_code)]
    account_to_label: HashMap<String, u32>,
}

impl Predictor for MultinomialNBPredictor {
    fn train(examples: &[TrainingExample]) -> Self {
        // Build feature extractor from training data (smart preprocessing)
        let feature_extractor = FeatureExtractor::fit(examples);

        // Build label mapping (account -> integer)
        let mut unique_accounts: Vec<String> = examples
            .iter()
            .map(|ex| ex.target_account.to_string())
            .collect();
        unique_accounts.sort();
        unique_accounts.dedup();

        let label_to_account = unique_accounts;
        let account_to_label: HashMap<String, u32> = label_to_account
            .iter()
            .enumerate()
            .map(|(idx, acc)| (acc.clone(), idx as u32))
            .collect();

        // Extract features
        let feature_matrix = feature_extractor.transform_batch(examples);

        // Convert to DenseMatrix<u32> (MultinomialNB requires unsigned types for counts)
        let mut data_2d: Vec<Vec<u32>> = Vec::new();
        for row in feature_matrix {
            let row_u32: Vec<u32> = row.into_iter().map(|v| v as u32).collect();
            data_2d.push(row_u32);
        }

        let x = DenseMatrix::from_2d_vec(&data_2d).expect("Failed to create feature matrix");

        // Build labels
        let y: Vec<u32> = examples
            .iter()
            .map(|ex| {
                *account_to_label
                    .get(&ex.target_account.to_string())
                    .expect("Account not in label map")
            })
            .collect();

        // Train Multinomial Naive Bayes
        let classifier = MultinomialNB::fit(&x, &y, Default::default())
            .expect("Failed to train Multinomial NB model");

        Self {
            classifier,
            feature_extractor,
            label_to_account,
            account_to_label,
        }
    }

    fn predict(&self, input: &PredictionInput) -> Option<Account> {
        let dummy_example = TrainingExample {
            source_account: input.source_account.clone(),
            payee: input.payee.clone(),
            narration: input.narration.clone(),
            target_account: "Expenses:Unknown".parse().unwrap(),
        };

        let features = self.feature_extractor.transform(&dummy_example);
        let features_u32: Vec<u32> = features.into_iter().map(|v| v as u32).collect();
        let x = DenseMatrix::from_2d_vec(&vec![features_u32]).ok()?;

        let predictions = self.classifier.predict(&x).ok()?;
        let label = predictions[0];
        let account_str = self.label_to_account.get(label as usize)?;
        account_str.parse().ok()
    }

    fn name(&self) -> &'static str {
        "MultinomialNB"
    }
}
