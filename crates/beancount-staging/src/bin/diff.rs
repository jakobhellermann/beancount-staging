use anyhow::Result;
use beancount_staging::reconcile::{ReconcileConfig, ReconcileItem};

fn main() -> Result<()> {
    let journal_paths = &[
        "src/transactions.beancount",
        "src/ignored.beancount",
        "src/balance.beancount",
    ];
    let staging_paths = &["extracted.beancount"];
    let results = ReconcileConfig::new(journal_paths, staging_paths).reconcile()?;

    for item in results {
        match item {
            ReconcileItem::OnlyInJournal(directive) => {
                dbg!("only journal");
                println!("{}", directive);
            }
            ReconcileItem::OnlyInStaging(directive) => {
                dbg!("only staging");
                println!("{}", directive);
            }
        }
    }

    Ok(())
}
