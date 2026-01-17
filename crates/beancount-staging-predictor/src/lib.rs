pub mod features;
pub mod predictor;
pub mod preprocessing;
pub mod training;

pub use beancount_staging::{Decimal, Directive, Result, Transaction};
pub use predictor::{
    DecisionTreePredictor, MultinomialNBPredictor, PayeeFrequencyPredictor, Predictor,
    RandomForestPredictor,
};

use beancount_parser::Account;

#[derive(Debug, Clone)]
pub struct TrainingExample {
    pub source_account: Account,
    pub payee: Option<String>,
    pub narration: String,
    pub target_account: Account,
}

#[derive(Debug, Clone)]
pub struct PredictionInput {
    pub source_account: Account,
    pub payee: Option<String>,
    pub narration: String,
}

impl From<&TrainingExample> for PredictionInput {
    fn from(example: &TrainingExample) -> Self {
        PredictionInput {
            source_account: example.source_account.clone(),
            payee: example.payee.clone(),
            narration: example.narration.clone(),
        }
    }
}
