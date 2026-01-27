// Shared evaluation utilities for binaries
// Some fields/methods unused in some binaries but used in others
#![allow(dead_code)]

use beancount_staging_predictor::predictor::Predictor;
use beancount_staging_predictor::{PredictionInput, TrainingExample};

pub fn train_test_split(
    examples: &[TrainingExample],
    train_ratio: f64,
) -> (Vec<TrainingExample>, Vec<TrainingExample>) {
    let split_idx = (examples.len() as f64 * train_ratio).floor() as usize;

    let train = examples[..split_idx].to_vec();
    let test = examples[split_idx..].to_vec();

    (train, test)
}

pub fn evaluate(
    predictor: &impl Predictor,
    test_examples: &[TrainingExample],
) -> EvaluationMetrics {
    let mut correct = 0;
    let mut total = 0;
    let mut predictions_made = 0;
    let mut no_prediction = 0;

    let mut confusion: Vec<(String, String, String)> = Vec::new();

    for example in test_examples {
        total += 1;

        let input = PredictionInput::from(example);

        match predictor.predict(&input) {
            Some(predicted) => {
                predictions_made += 1;

                if predicted.to_string() == example.target_account.to_string() {
                    correct += 1;
                } else {
                    confusion.push((
                        example.target_account.to_string(),
                        predicted.to_string(),
                        example.payee.clone().unwrap_or_else(|| "<no payee>".into()),
                    ));
                }
            }
            None => {
                no_prediction += 1;
                confusion.push((
                    example.target_account.to_string(),
                    "<no prediction>".into(),
                    example.payee.clone().unwrap_or_else(|| "<no payee>".into()),
                ));
            }
        }
    }

    EvaluationMetrics {
        total,
        correct,
        predictions_made,
        no_prediction,
        accuracy: if predictions_made > 0 {
            correct as f64 / predictions_made as f64
        } else {
            0.0
        },
        coverage: predictions_made as f64 / total as f64,
        confusion_samples: confusion.into_iter().take(5).collect(),
    }
}

pub struct EvaluationMetrics {
    total: usize,
    correct: usize,
    predictions_made: usize,
    no_prediction: usize,
    pub accuracy: f64,
    coverage: f64,
    confusion_samples: Vec<(String, String, String)>,
}

impl EvaluationMetrics {
    pub fn report(&self, predictor_name: &str) -> String {
        let mut report = format!(
            "=== Evaluation Results ===\n\
             Predictor: {}\n\
             Total test examples: {}\n\
             Predictions made: {} ({:.1}% coverage)\n\
             No prediction: {}\n\
             Correct predictions: {}\n\
             Accuracy (of predictions made): {:.1}%\n\
             Overall accuracy: {:.1}%\n",
            predictor_name,
            self.total,
            self.predictions_made,
            self.coverage * 100.0,
            self.no_prediction,
            self.correct,
            self.accuracy * 100.0,
            (self.correct as f64 / self.total as f64) * 100.0,
        );

        if !self.confusion_samples.is_empty() {
            report.push_str("\n=== Sample Errors (first 5) ===\n");
            for (expected, predicted, payee) in &self.confusion_samples {
                report.push_str(&format!(
                    "  Payee: {}\n    Expected: {}\n    Predicted: {}\n\n",
                    payee, expected, predicted
                ));
            }
        }

        report
    }
}
