use std::path::PathBuf;

use anstyle::{AnsiColor, Color, Style};
use anyhow::Result;
use beancount_parser::DirectiveContent;
use beancount_staging::Directive;
use beancount_staging::reconcile::{ReconcileConfig, ReconcileItem};
use clap::{Args as ClapArgs, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "diff")]
#[command(about = "Compare journal and staging beancount files")]
struct Args {
    #[command(flatten)]
    files: FileArgs,

    #[command(subcommand)]
    command: Commands,
}

#[derive(ClapArgs)]
struct FileArgs {
    /// Journal file path
    #[arg(short, long, required = true)]
    journal: Vec<PathBuf>,

    /// Staging file path
    #[arg(short, long, required = true)]
    staging: Vec<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show differences between journal and staging files
    Show,
    /// Interactively review and stage transactions
    Review,
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::Show => show_diff(args.files.journal, args.files.staging),
        Commands::Review => review_interactive(args.files.journal, args.files.staging),
    }
}

fn show_diff(journal: Vec<PathBuf>, staging: Vec<PathBuf>) -> Result<()> {
    let results = ReconcileConfig::new(journal, staging).reconcile()?;

    let journal_style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Yellow)));
    let staging_style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)));
    let reset = Style::new();

    let mut journal_count = 0;
    let mut staging_count = 0;

    for item in &results {
        match item {
            ReconcileItem::OnlyInJournal(directive) => {
                if let DirectiveContent::Open(_) = directive.content {
                    continue;
                }

                println!("{journal_style}━━━ Only in Journal ━━━{reset}");
                println!("{}", directive);
                println!();
                journal_count += 1;
            }
            ReconcileItem::OnlyInStaging(directive) => {
                println!("{staging_style}━━━ Only in Staging (needs review) ━━━{reset}");
                println!("{}", directive);
                println!();
                staging_count += 1;
            }
        }
    }

    // Summary
    if results.is_empty() {
        println!("✓ All transactions match!");
    } else {
        println!("{}━━━ Summary ━━━{}", Style::new().bold(), reset);
        if journal_count > 0 {
            println!("  {journal_style}{journal_count}{reset} transaction(s) only in journal");
        }
        if staging_count > 0 {
            println!(
                "  {staging_style}{staging_count}{reset} transaction(s) only in staging (need review)"
            );
        }
    }

    Ok(())
}

fn commit_transaction(
    directive: &Directive,
    expense_account: &str,
    journal_path: &PathBuf,
) -> Result<()> {
    use anyhow::Context;
    use std::fs::OpenOptions;
    use std::io::Write;

    // Clone and modify the directive
    let mut modified_directive = directive.clone();

    // Modify the transaction: change flag to * and add balancing posting
    if let beancount_parser::DirectiveContent::Transaction(ref mut txn) = modified_directive.content
    {
        // Change flag from ! to *
        txn.flag = Some('*');

        // Add balancing posting with expense account (no amount - beancount infers it)
        let account: beancount_parser::Account = expense_account
            .parse()
            .with_context(|| format!("Failed to parse account name: '{}'", expense_account))?;
        txn.postings.push(beancount_parser::Posting::new(account));
    }

    // Open journal file in append mode
    let mut file = OpenOptions::new().append(true).open(journal_path)?;

    // Format and write with tabs replaced by spaces
    let content = format!("{}", modified_directive).replace('\t', "    ");
    writeln!(file, "\n{}", content)?;

    Ok(())
}

fn review_interactive(journal: Vec<PathBuf>, staging: Vec<PathBuf>) -> Result<()> {
    let results = ReconcileConfig::new(journal.clone(), staging).reconcile()?;

    // Filter only staging items
    let staging_items: Vec<_> = results
        .iter()
        .filter_map(|item| match item {
            ReconcileItem::OnlyInStaging(directive) => Some(directive),
            _ => None,
        })
        .collect();

    if staging_items.is_empty() {
        println!("No items to review in staging!");
        return Ok(());
    }

    // Initialize terminal
    let mut terminal = ratatui::init();

    // Run the interactive loop and ensure terminal is restored
    let result = run_review_loop(&mut terminal, staging_items, &journal[0]);

    ratatui::restore();

    result
}

fn run_review_loop(
    terminal: &mut ratatui::DefaultTerminal,
    mut staging_items: Vec<&beancount_parser::Directive<beancount_staging::Decimal>>,
    journal_path: &PathBuf,
) -> Result<()> {
    use crossterm::event::{self, Event, KeyCode, KeyEventKind};
    use std::time::Duration;

    let mut current_index = 0;
    let mut expense_accounts: Vec<Option<String>> = vec![None; staging_items.len()];
    let mut input_mode = false;
    let mut input_buffer = String::new();

    loop {
        terminal.draw(|frame| {
            use ratatui::layout::{Constraint, Layout};

            let area = frame.area();

            // Split area for transaction display and input
            let chunks = Layout::vertical([Constraint::Min(3), Constraint::Length(3)]).split(area);

            let directive = staging_items[current_index];
            let content = format!("{}", directive).replace('\t', "    ");

            let mode_hint = if input_mode {
                "ESC to cancel"
            } else {
                "e to edit"
            };
            let title = format!(
                "Review Staging ({}/{}) [← → navigate | {} | q quit]",
                current_index + 1,
                staging_items.len(),
                mode_hint
            );

            let paragraph = ratatui::widgets::Paragraph::new(content)
                .block(ratatui::widgets::Block::bordered().title(title));

            frame.render_widget(paragraph, chunks[0]);

            // Show input field
            let account_display = if input_mode {
                input_buffer.clone()
            } else {
                expense_accounts[current_index]
                    .as_deref()
                    .unwrap_or("")
                    .to_string()
            };

            let input_title = if input_mode {
                "Expense Account (Enter to save)"
            } else {
                "Expense Account"
            };

            let input = ratatui::widgets::Paragraph::new(account_display)
                .block(ratatui::widgets::Block::bordered().title(input_title));

            frame.render_widget(input, chunks[1]);
        })?;

        // Poll for events
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            if input_mode {
                // Input mode: handle text entry
                match key.code {
                    KeyCode::Enter => {
                        // Save the account
                        expense_accounts[current_index] = Some(input_buffer.clone());
                        input_buffer.clear();
                        input_mode = false;
                    }
                    KeyCode::Esc => {
                        // Cancel input
                        input_buffer.clear();
                        input_mode = false;
                    }
                    KeyCode::Char(c) => {
                        input_buffer.push(c);
                    }
                    KeyCode::Backspace => {
                        input_buffer.pop();
                    }
                    _ => {}
                }
            } else {
                // Navigation mode
                match key.code {
                    KeyCode::Char('q') => break Ok(()),
                    KeyCode::Char('c')
                        if key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        break Ok(());
                    }
                    KeyCode::Char('e') => {
                        // Enter input mode
                        input_mode = true;
                        input_buffer = expense_accounts[current_index].clone().unwrap_or_default();
                    }
                    KeyCode::Enter => {
                        // Commit transaction if expense account is set
                        if let Some(expense_account) = &expense_accounts[current_index] {
                            let directive = staging_items[current_index];
                            match commit_transaction(directive, expense_account, journal_path) {
                                Ok(()) => {
                                    // Remove from list
                                    staging_items.remove(current_index);
                                    expense_accounts.remove(current_index);

                                    // Check if we're done
                                    if staging_items.is_empty() {
                                        break Ok(());
                                    }

                                    // Adjust index if needed
                                    if current_index >= staging_items.len() {
                                        current_index = staging_items.len() - 1;
                                    }
                                }
                                Err(e) => {
                                    // On error, just return - caller will restore terminal
                                    return Err(e);
                                }
                            }
                        }
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        current_index = (current_index + 1) % staging_items.len();
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        current_index = if current_index == 0 {
                            staging_items.len() - 1
                        } else {
                            current_index - 1
                        };
                    }
                    _ => {}
                }
            }
        }
    }
}
