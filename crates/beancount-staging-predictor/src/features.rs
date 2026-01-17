/// Feature extraction for machine learning predictors
use crate::{
    TrainingExample,
    preprocessing::{Alpha, Preprocessor},
};
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;

#[derive(Debug, Clone)]
pub struct FeatureExtractor<P: Preprocessor = Alpha> {
    /// Vocabulary: maps feature name to index
    vocabulary: HashMap<String, usize>,
    /// Number of features
    feature_count: usize,
    _preprocessor: PhantomData<P>,
}

impl<P: Preprocessor + Default> FeatureExtractor<P> {
    /// Build vocabulary from training examples
    pub fn fit(examples: &[TrainingExample]) -> Self {
        let preprocessor = P::default();
        let mut features = HashSet::new();

        for example in examples {
            let extracted = Self::extract_feature_names(example, &preprocessor);
            features.extend(extracted);
        }

        // Sort features for deterministic ordering
        let mut sorted_features: Vec<_> = features.into_iter().collect();
        sorted_features.sort();

        let vocabulary: HashMap<String, usize> = sorted_features
            .into_iter()
            .enumerate()
            .map(|(idx, feat)| (feat, idx))
            .collect();

        let feature_count = vocabulary.len();

        Self {
            vocabulary,
            feature_count,
            _preprocessor: PhantomData,
        }
    }

    fn extract_feature_names(example: &TrainingExample, preprocessor: &P) -> Vec<String> {
        let mut features = Vec::new();

        // Source account feature
        features.push(format!("account:{}", example.source_account));

        // Payee features
        if let Some(payee) = &example.payee {
            features.push(format!("payee:{}", payee));

            // Payee word unigrams
            for word in payee.split_whitespace() {
                if word.len() > 2 {
                    features.push(format!("payee_word:{}", word.to_lowercase()));
                }
            }
        }

        // Narration features
        let narration = preprocessor.preprocess(&example.narration);
        let words: Vec<_> = narration.split_whitespace().collect();

        // Unigrams
        for word in &words {
            if word.len() > 2 {
                features.push(format!("desc:{}", word));
            }
        }

        // Bigrams
        for window in words.windows(2) {
            features.push(format!("desc:{} {}", window[0], window[1]));
        }

        features
    }

    /// Transform an example into a feature vector
    pub fn transform(&self, example: &TrainingExample) -> Vec<f64> {
        let preprocessor = P::default();
        let mut feature_vec = vec![0.0; self.feature_count];

        let feature_names = Self::extract_feature_names(example, &preprocessor);

        for name in feature_names {
            if let Some(&idx) = self.vocabulary.get(&name) {
                feature_vec[idx] = 1.0;
            }
        }

        feature_vec
    }

    /// Transform multiple examples into a feature matrix
    pub fn transform_batch(&self, examples: &[TrainingExample]) -> Vec<Vec<f64>> {
        examples.iter().map(|ex| self.transform(ex)).collect()
    }

    pub fn feature_count(&self) -> usize {
        self.feature_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preprocessing::Smart;

    fn create_example(
        source: &str,
        payee: Option<&str>,
        narration: &str,
        target: &str,
    ) -> TrainingExample {
        TrainingExample {
            source_account: source.parse().unwrap(),
            payee: payee.map(|s| s.to_string()),
            narration: narration.to_string(),
            target_account: target.parse().unwrap(),
        }
    }

    #[test]
    fn test_feature_extractor_basic() {
        let examples = vec![
            create_example(
                "Assets:Checking",
                Some("REWE"),
                "Groceries",
                "Expenses:Groceries",
            ),
            create_example(
                "Assets:Checking",
                Some("LIDL"),
                "More groceries",
                "Expenses:Groceries",
            ),
        ];

        let extractor = FeatureExtractor::<Smart>::fit(&examples);

        // Should have features for:
        // - account:Assets:Checking
        // - payee:REWE, payee:LIDL
        // - payee_word:rewe, payee_word:lidl
        // - desc:groceries, desc:more
        // - desc:more groceries
        assert!(extractor.feature_count > 0);

        let vec = extractor.transform(&examples[0]);
        assert_eq!(vec.len(), extractor.feature_count);

        // Should have some non-zero features
        let non_zero_count = vec.iter().filter(|&&v| v > 0.0).count();
        assert!(non_zero_count > 0);
    }

    #[test]
    fn test_feature_extractor_with_preprocessing() {
        let examples = vec![create_example(
            "Assets:Checking",
            Some("LIDL"),
            "LIDL SAGT DANKE/Essen 10.01.2026 um 20:34:34 REF 101002/260045",
            "Expenses:Groceries",
        )];

        let extractor = FeatureExtractor::<Smart>::fit(&examples);
        let vec = extractor.transform(&examples[0]);

        // The feature vector should not contain noise (dates, times, refs)
        // But should contain meaningful words (lidl, sagt, danke, essen)

        // We can't easily test the exact features without exposing vocabulary,
        // but we can test that transformation works
        assert!(vec.len() > 0);
        assert!(vec.iter().any(|&v| v > 0.0));
    }

    #[test]
    fn test_feature_extractor_batch_transform() {
        let examples = vec![
            create_example(
                "Assets:Checking",
                Some("REWE"),
                "Groceries",
                "Expenses:Groceries",
            ),
            create_example(
                "Liabilities:CreditCard",
                Some("Amazon"),
                "Books",
                "Expenses:Shopping",
            ),
        ];

        let extractor = FeatureExtractor::<Smart>::fit(&examples);
        let matrix = extractor.transform_batch(&examples);

        assert_eq!(matrix.len(), 2);
        assert_eq!(matrix[0].len(), extractor.feature_count);
        assert_eq!(matrix[1].len(), extractor.feature_count);

        // Each row should have different features activated
        assert_ne!(matrix[0], matrix[1]);
    }

    #[test]
    fn test_feature_extractor_unseen_features() {
        let train_examples = vec![create_example(
            "Assets:Checking",
            Some("REWE"),
            "Groceries",
            "Expenses:Groceries",
        )];

        let extractor = FeatureExtractor::<Smart>::fit(&train_examples);

        // Test example with unseen features
        let test_example = create_example(
            "Liabilities:CreditCard",
            Some("UnknownPayee"),
            "Unknown description",
            "Expenses:Unknown",
        );

        let vec = extractor.transform(&test_example);

        // Should still produce a vector of correct size
        assert_eq!(vec.len(), extractor.feature_count);

        // Should be all zeros (no matching features)
        assert_eq!(vec.iter().filter(|&&v| v > 0.0).count(), 0);
    }
}
