use anyhow::Result;
use beancount_staging_predictor::{
    preprocessing::preprocess_text, training::extract_training_examples,
};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "inspect-preprocessing")]
#[command(about = "Inspect text preprocessing on real transaction data")]
struct Args {
    #[arg(short = 'j', long = "journal", required = true)]
    journal_paths: Vec<PathBuf>,

    #[arg(long, default_value = "20")]
    limit: usize,
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

    println!("Showing {} sample preprocessed narrations:\n", args.limit);
    println!("{:<50} | {:<50}", "ORIGINAL", "PREPROCESSED");
    println!("{}", "=".repeat(103));

    for (i, example) in examples.iter().take(args.limit).enumerate() {
        let original = &example.narration;
        let preprocessed = preprocess_text(original);

        // Truncate for display
        let orig_display = if original.len() > 48 {
            format!("{}...", &original[..48])
        } else {
            original.clone()
        };

        let prep_display = if preprocessed.len() > 48 {
            format!("{}...", &preprocessed[..48])
        } else {
            preprocessed.clone()
        };

        println!("{:<50} | {:<50}", orig_display, prep_display);

        // Check if preprocessing removed too much
        if preprocessed.split_whitespace().count() < 2 && original.split_whitespace().count() > 3 {
            eprintln!(
                "⚠️  Line {}: Preprocessing removed too much content!",
                i + 1
            );
        }

        // Check if numbers remain (might be wanted or unwanted)
        if preprocessed.chars().any(|c| c.is_numeric()) {
            let numbers: String = preprocessed.chars().filter(|c| c.is_numeric()).collect();
            if numbers.len() >= 4 {
                eprintln!(
                    "ℹ️  Line {}: Still contains numbers: {}",
                    i + 1,
                    preprocessed
                );
            }
        }
    }

    // Stats
    let mut total_too_short = 0;
    let mut total_with_long_numbers = 0;

    for example in &examples {
        let preprocessed = preprocess_text(&example.narration);
        if preprocessed.split_whitespace().count() < 2
            && example.narration.split_whitespace().count() > 3
        {
            total_too_short += 1;
        }

        let numbers: String = preprocessed.chars().filter(|c| c.is_numeric()).collect();
        if numbers.len() >= 4 {
            total_with_long_numbers += 1;
        }
    }

    println!("\n{}", "=".repeat(103));
    println!("Statistics across {} examples:", examples.len());
    println!("  - Over-preprocessed (too short): {}", total_too_short);
    println!(
        "  - Still contain 4+ digit numbers: {}",
        total_with_long_numbers
    );

    Ok(())
}
