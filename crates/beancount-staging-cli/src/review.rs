use std::path::{Path, PathBuf};

use anyhow::Result;
use beancount_staging::{
    Directive,
    reconcile::{ReconcileConfig, ReconcileItem, StagingSource},
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::time::Duration;

pub fn review_interactive(journal: Vec<PathBuf>, staging_source: StagingSource) -> Result<()> {
    let state = ReconcileConfig::new(journal.clone(), staging_source).read()?;
    let results = state.reconcile()?;

    // Filter only staging items
    let staging_items: Vec<_> = results
        .iter()
        .filter_map(|item| match *item {
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
    mut staging_items: Vec<&Directive>,
    journal_path: &Path,
) -> Result<()> {
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
                            match beancount_staging::commit_transaction(
                                directive,
                                Some(expense_account.as_str()),
                                None, // payee unchanged
                                None, // narration unchanged
                                beancount_staging::SourceMetaTarget::Transaction,
                                journal_path,
                            ) {
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
