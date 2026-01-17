# Beancount Staging Predictor

Account prediction system for automatically suggesting expense accounts for imported transactions.

## Architecture

### Predictor Trait

All predictors implement the `Predictor` trait:

```rust
pub trait Predictor {
    fn train(examples: &[TrainingExample]) -> Self where Self: Sized;
    fn predict(&self, input: &PredictionInput) -> Option<Account>;
    fn name(&self) -> &'static str;
}
```

### Available Predictors

#### PayeeFrequency (baseline)

Strategy: Most frequent account by payee

- For each payee, learns which target accounts were used historically
- Predicts the most common target account for that payee
- Falls back to most common account for source when payee is unknown

**Performance on real data:**

- 81.4% accuracy on predictions made
- 97.0% coverage (only 3% no prediction)
- 79.0% overall accuracy

## Usage

### Command Line

```bash
# Use default predictor (PayeeFrequency)
just predict-eval

# Explicit predictor selection
cargo run -p beancount-staging-predictor --bin beancount-predictor-eval -- \
    -j ~/finances/src/transactions.beancount \
    -j ~/finances/journal.beancount \
    --predictor payee-frequency

# Custom train/test split
just predict-eval --train-ratio 0.9
```

### As a Library

```rust
use beancount_staging_predictor::{
    Predictor,
    predictor::PayeeFrequencyPredictor,
    training::extract_training_examples,
    evaluation::evaluate,
};

// Extract training data
let directives = beancount_staging::read_directives("journal.beancount")?;
let examples = extract_training_examples(&directives);

// Train predictor
let predictor = PayeeFrequencyPredictor::train(&examples);

// Make prediction
let input = PredictionInput {
    source_account: "Assets:Checking".parse()?,
    payee: Some("REWE".into()),
    narration: "Groceries".into(),
};

let predicted_account = predictor.predict(&input);
```

## Adding New Predictors

1. Create a new struct in `src/predictor.rs`
2. Implement the `Predictor` trait
3. Add to `PredictorType` enum in `src/bin/evaluate.rs`
4. Add match arm in `main()` to instantiate and evaluate

Example:

```rust
// In src/predictor.rs
pub struct MyNewPredictor {
    // ... fields
}

impl Predictor for MyNewPredictor {
    fn train(examples: &[TrainingExample]) -> Self {
        // ... training logic
    }

    fn predict(&self, input: &PredictionInput) -> Option<Account> {
        // ... prediction logic
    }

    fn name(&self) -> &'static str {
        "MyNewPredictor"
    }
}

// In src/bin/evaluate.rs
#[derive(Debug, Clone, clap::ValueEnum)]
enum PredictorType {
    PayeeFrequency,
    MyNewPredictor,  // Add here
}

// In main()
match args.predictor {
    PredictorType::PayeeFrequency => { /* ... */ }
    PredictorType::MyNewPredictor => {
        let predictor = MyNewPredictor::train(&train_examples);
        let metrics = evaluate(&predictor, &test_examples);
        println!("{}", metrics.report(predictor.name()));
    }
}
```

## Next Steps

Future predictor implementations (from TRAINING_IMPLEMENTATION_GUIDE.md):

1. **Text Preprocessing**: Clean narration field (remove dates, IDs, noise)
2. **N-gram Features**: Extract unigrams and bigrams from descriptions
3. **ML Classifier**: Decision tree or random forest using `smartcore`
4. **Hybrid Approach**: Combine frequency baseline with ML for better accuracy

The goal is to improve beyond the 81% baseline, especially for ambiguous payees like PayPal and Amazon.
