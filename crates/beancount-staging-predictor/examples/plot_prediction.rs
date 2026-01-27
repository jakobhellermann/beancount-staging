mod shared;

use anyhow::Result;
use beancount_staging_predictor::{
    predictor::{
        DecisionTreePredictor, MultinomialNBPredictor, PayeeFrequencyPredictor, Predictor,
        RandomForestPredictor,
    },
    preprocessing::{Alpha, Raw, Smart},
    training::extract_training_examples,
};
use clap::Parser;
use rand::SeedableRng;
use rand::seq::SliceRandom;
use shared::evaluation::{evaluate, train_test_split};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "plot-prediction")]
#[command(about = "Generate learning curve data comparing predictor strategies")]
struct Args {
    #[arg(short = 'j', long = "journal", required = true)]
    journal_paths: Vec<PathBuf>,

    #[arg(long, default_value = "0.8")]
    train_ratio: f64,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Read all directives
    let mut all_directives = Vec::new();
    for path in &args.journal_paths {
        let directives = beancount_staging::read_directives(path)?;
        all_directives.extend(directives);
    }

    // Extract training examples
    let examples = extract_training_examples(&all_directives);
    let (train_examples, test_examples) = train_test_split(&examples, args.train_ratio);

    eprintln!(
        "Total examples: {}, Train: {}, Test: {}",
        examples.len(),
        train_examples.len(),
        test_examples.len()
    );
    eprintln!();
    println!(
        "training_size,payee_freq_accuracy,dt_alpha_accuracy,dt_smart_accuracy,dt_raw_accuracy,dt_shuffled_accuracy,random_forest_accuracy,multinomial_nb_accuracy,payee_freq_time_ms,dt_alpha_time_ms,dt_smart_time_ms,dt_raw_time_ms,dt_shuffled_time_ms,random_forest_time_ms,multinomial_nb_time_ms"
    );

    // Test with increasing amounts of training data
    // Step size: 10 for first 100, then 25 until 500, then 50
    let max_size = train_examples.len();
    let mut sample_sizes: Vec<usize> = Vec::new();

    // 10-step increments up to 100
    for size in (10..=100).step_by(10) {
        if size <= max_size {
            sample_sizes.push(size);
        }
    }

    // 25-step increments from 125 to 500
    for size in (125..=500).step_by(25) {
        if size <= max_size {
            sample_sizes.push(size);
        }
    }

    // 50-step increments from 550 onwards
    let mut size = 550;
    while size < max_size {
        sample_sizes.push(size);
        size += 50;
    }

    // Always include the full training set
    if !sample_sizes.contains(&max_size) {
        sample_sizes.push(max_size);
    }

    for &size in &sample_sizes {
        if size > train_examples.len() {
            break;
        }

        let subset = &train_examples[..size];

        // Train and evaluate PayeeFrequency
        let start = Instant::now();
        let payee_freq = PayeeFrequencyPredictor::train(subset);
        let payee_train_time = start.elapsed().as_millis();
        let payee_metrics = evaluate(&payee_freq, &test_examples);

        // Train and evaluate DecisionTree with Alpha preprocessing
        let start = Instant::now();
        let dt_alpha = DecisionTreePredictor::<Alpha>::train(subset);
        let dt_alpha_train_time = start.elapsed().as_millis();
        let dt_alpha_metrics = evaluate(&dt_alpha, &test_examples);

        // Train and evaluate DecisionTree with Smart preprocessing
        let start = Instant::now();
        let dt_smart = DecisionTreePredictor::<Smart>::train(subset);
        let dt_smart_train_time = start.elapsed().as_millis();
        let dt_smart_metrics = evaluate(&dt_smart, &test_examples);

        // Train and evaluate DecisionTree with Raw preprocessing
        let start = Instant::now();
        let dt_raw = DecisionTreePredictor::<Raw>::train(subset);
        let dt_raw_train_time = start.elapsed().as_millis();
        let dt_raw_metrics = evaluate(&dt_raw, &test_examples);

        // Train and evaluate DecisionTree with SHUFFLED training data (alpha preprocessing)
        let mut shuffled_subset = subset.to_vec();
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        shuffled_subset.shuffle(&mut rng);
        let start = Instant::now();
        let dt_shuffled = DecisionTreePredictor::<Alpha>::train(&shuffled_subset);
        let dt_shuffled_train_time = start.elapsed().as_millis();
        let dt_shuffled_metrics = evaluate(&dt_shuffled, &test_examples);

        // Train and evaluate Random Forest (10 trees, Smart preprocessing)
        let start = Instant::now();
        let random_forest = RandomForestPredictor::train(subset);
        let rf_train_time = start.elapsed().as_millis();
        let rf_metrics = evaluate(&random_forest, &test_examples);

        // Train and evaluate Multinomial Naive Bayes (Smart preprocessing)
        let start = Instant::now();
        let multinomial_nb = MultinomialNBPredictor::train(subset);
        let nb_train_time = start.elapsed().as_millis();
        let nb_metrics = evaluate(&multinomial_nb, &test_examples);

        println!(
            "{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{},{},{},{},{},{},{}",
            size,
            payee_metrics.accuracy * 100.0,
            dt_alpha_metrics.accuracy * 100.0,
            dt_smart_metrics.accuracy * 100.0,
            dt_raw_metrics.accuracy * 100.0,
            dt_shuffled_metrics.accuracy * 100.0,
            rf_metrics.accuracy * 100.0,
            nb_metrics.accuracy * 100.0,
            payee_train_time,
            dt_alpha_train_time,
            dt_smart_train_time,
            dt_raw_train_time,
            dt_shuffled_train_time,
            rf_train_time,
            nb_train_time
        );
    }

    Ok(())
}
