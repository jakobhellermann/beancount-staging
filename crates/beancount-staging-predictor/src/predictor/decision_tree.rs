use super::Predictor;
use crate::{
    PredictionInput, TrainingExample,
    features::FeatureExtractor,
    preprocessing::{Alpha, Preprocessor},
};
use beancount_parser::Account;
use smartcore::linalg::basic::matrix::DenseMatrix;
use smartcore::tree::decision_tree_classifier::{
    DecisionTreeClassifier, DecisionTreeClassifierParameters,
};
use std::collections::HashMap;
use std::marker::PhantomData;

/// N-gram ML predictor using decision tree classifier
/// Generic over preprocessing strategy
pub struct DecisionTreePredictor<P: Preprocessor = Alpha> {
    classifier: DecisionTreeClassifier<f64, i32, DenseMatrix<f64>, Vec<i32>>,
    feature_extractor: FeatureExtractor<P>,
    label_to_account: Vec<String>,
    #[allow(dead_code)]
    account_to_label: HashMap<String, i32>,
    _preprocessor: PhantomData<P>,
}

impl<P: Preprocessor + Default> DecisionTreePredictor<P> {
    pub fn stats(&self) -> MLPredictorStats {
        MLPredictorStats {
            n_features: self.feature_extractor.feature_count(),
            n_classes: self.label_to_account.len(),
        }
    }
}

impl<P: Preprocessor + Default> Predictor for DecisionTreePredictor<P> {
    fn train(examples: &[TrainingExample]) -> Self {
        // Build feature extractor
        let feature_extractor = FeatureExtractor::<P>::fit(examples);

        // Build label mapping (account -> integer)
        let mut unique_accounts: Vec<String> = examples
            .iter()
            .map(|ex| ex.target_account.to_string())
            .collect();
        unique_accounts.sort();
        unique_accounts.dedup();

        let label_to_account = unique_accounts;
        let account_to_label: HashMap<String, i32> = label_to_account
            .iter()
            .enumerate()
            .map(|(idx, acc)| (acc.clone(), idx as i32))
            .collect();

        // Extract features
        let feature_matrix = feature_extractor.transform_batch(examples);

        // Convert to DenseMatrix
        let mut data_2d: Vec<Vec<f64>> = Vec::new();
        for row in feature_matrix {
            data_2d.push(row);
        }

        let x = DenseMatrix::from_2d_vec(&data_2d).expect("Failed to create feature matrix");

        // Build labels
        let y: Vec<i32> = examples
            .iter()
            .map(|ex| {
                *account_to_label
                    .get(&ex.target_account.to_string())
                    .expect("Account not in label map")
            })
            .collect();

        // Train decision tree
        let params = DecisionTreeClassifierParameters::default();
        let classifier =
            DecisionTreeClassifier::fit(&x, &y, params).expect("Failed to train model");

        Self {
            classifier,
            feature_extractor,
            label_to_account,
            account_to_label,
            _preprocessor: PhantomData,
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

        if features.iter().all(|&f| f == 0.0) {
            return None;
        }

        let x =
            DenseMatrix::from_2d_vec(&vec![features]).expect("Failed to create prediction matrix");

        let predictions = self.classifier.predict(&x).ok()?;
        let label = predictions[0];

        let account_str = self.label_to_account.get(label as usize)?;
        account_str.parse().ok()
    }

    fn name(&self) -> &'static str {
        std::any::type_name::<P>()
            .split("::")
            .last()
            .unwrap_or("DecisionTree")
    }
}

#[derive(Debug)]
pub struct MLPredictorStats {
    pub n_features: usize,
    pub n_classes: usize,
}
