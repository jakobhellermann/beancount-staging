mod shared;

use anyhow::Result;
use beancount_staging_predictor::{
    predictor::{DecisionTreePredictor, PayeeFrequencyPredictor, Predictor},
    preprocessing::Alpha,
    training::extract_training_examples,
};
use clap::Parser;
use shared::evaluation::{evaluate, train_test_split};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "beancount-predictor-eval")]
#[command(about = "Evaluate account prediction on beancount journal files")]
struct Args {
    #[arg(short = 'j', long = "journal", required = true)]
    journal_paths: Vec<PathBuf>,

    #[arg(long, default_value = "0.8")]
    train_ratio: f64,

    #[arg(short, long)]
    verbose: bool,

    #[arg(long, default_value = "ngram-ml")]
    predictor: PredictorType,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum PredictorType {
    PayeeFrequency,
    NgramMl,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("Reading journal files...");
    let mut all_directives = Vec::new();
    for path in &args.journal_paths {
        let directives = beancount_staging::read_directives(path)?;
        println!("  - {}: {} directives", path.display(), directives.len());
        all_directives.extend(directives);
    }

    println!("\nExtracting training examples...");
    let examples = extract_training_examples(&all_directives);
    println!("  Found {} training examples", examples.len());

    if examples.is_empty() {
        anyhow::bail!(
            "No training examples found. Make sure journal files contain complete transactions."
        );
    }

    println!("\nSplitting data (train ratio: {})...", args.train_ratio);
    let (train_examples, test_examples) = train_test_split(&examples, args.train_ratio);
    println!("  Train: {} examples", train_examples.len());
    println!("  Test: {} examples", test_examples.len());

    if test_examples.is_empty() {
        anyhow::bail!("No test examples after split. Need more data.");
    }

    match args.predictor {
        PredictorType::PayeeFrequency => {
            println!("\nTraining PayeeFrequency predictor...");
            let predictor = PayeeFrequencyPredictor::train(&train_examples);
            let stats = predictor.stats();
            println!("  Learned {} unique payees", stats.unique_payees);
            println!("  Learned {} unique source accounts", stats.unique_sources);

            println!("\nEvaluating on test set...");
            let metrics = evaluate(&predictor, &test_examples);

            println!("\n{}", metrics.report(predictor.name()));

            if metrics.accuracy < 0.5 {
                anyhow::bail!("Accuracy below 50% - predictor needs improvement");
            }
        }
        PredictorType::NgramMl => {
            println!("\nTraining NgramML predictor (decision tree with Alpha preprocessing)...");
            let predictor = DecisionTreePredictor::<Alpha>::train(&train_examples);
            let stats = predictor.stats();
            println!("  Extracted {} features", stats.n_features);
            println!("  Training for {} classes (accounts)", stats.n_classes);

            println!("\nEvaluating on test set...");
            let metrics = evaluate(&predictor, &test_examples);

            println!("\n{}", metrics.report(predictor.name()));

            if metrics.accuracy < 0.5 {
                anyhow::bail!("Accuracy below 50% - predictor needs improvement");
            }
        }
    }

    Ok(())
}
