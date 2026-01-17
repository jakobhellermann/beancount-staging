use crate::{PredictionInput, TrainingExample};
use beancount_parser::Account;

mod baseline;
mod decision_tree;
mod ensemble;
mod naive_bayes;

pub use baseline::{PayeeFrequencyPredictor, PredictorStats};
pub use decision_tree::{DecisionTreePredictor, MLPredictorStats};
pub use ensemble::RandomForestPredictor;
pub use naive_bayes::MultinomialNBPredictor;

pub trait Predictor {
    fn train(examples: &[TrainingExample]) -> Self
    where
        Self: Sized;

    fn predict(&self, input: &PredictionInput) -> Option<Account>;

    fn name(&self) -> &'static str;
}
