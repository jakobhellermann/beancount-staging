use anyhow::{Context, Result};

fn main() -> Result<()> {
    let file = std::env::args()
        .nth(1)
        .context("expected path to journal file")?;

    let directives = beancount_staging::read_directives(file)?;
    for directive in &directives {
        println!("{}\n", directive);
    }

    dbg!(directives.len());

    Ok(())
}
