use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::state::AppState;
use beancount_staging::Directive;

fn serialize_directive(index: usize, directive: &Directive) -> SerializedDirective {
    use beancount_parser::DirectiveContent;

    let transaction = match &directive.content {
        DirectiveContent::Transaction(txn) => {
            let postings = txn
                .postings
                .iter()
                .map(|p| {
                    let amount = p.amount.as_ref().map(|amt| SerializedAmount {
                        value: amt.value.to_string(),
                        currency: amt.currency.to_string(),
                    });

                    SerializedPosting {
                        account: p.account.to_string(),
                        amount,
                        cost: p.cost.as_ref().map(|c| format!("{:?}", c)),
                        price: p.price.as_ref().map(|p| format!("{:?}", p)),
                    }
                })
                .collect();

            SerializedTransaction {
                date: directive.date.to_string(),
                flag: txn
                    .flag
                    .map(|f| f.to_string())
                    .unwrap_or_else(|| "*".to_string()),
                payee: txn.payee.clone(),
                narration: txn.narration.clone(),
                tags: txn.tags.iter().map(|t| t.to_string()).collect(),
                links: txn.links.iter().map(|l| l.to_string()).collect(),
                postings,
            }
        }
        _ => todo!("Only transactions staging are supported right now"),
    };

    SerializedDirective { index, transaction }
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, Json(self)).into_response()
    }
}

#[derive(Serialize)]
pub struct InitResponse {
    pub items: Vec<SerializedDirective>,
    pub current_index: usize,
    pub available_accounts: Vec<String>,
}

#[derive(Serialize)]
pub struct SerializedDirective {
    pub index: usize,
    pub transaction: SerializedTransaction,
}

#[derive(Serialize)]
pub struct SerializedTransaction {
    pub date: String,
    pub flag: String,
    pub payee: Option<String>,
    pub narration: Option<String>,
    pub tags: Vec<String>,
    pub links: Vec<String>,
    pub postings: Vec<SerializedPosting>,
}

#[derive(Serialize)]
pub struct SerializedPosting {
    pub account: String,
    pub amount: Option<SerializedAmount>,
    pub cost: Option<String>,
    pub price: Option<String>,
}

#[derive(Serialize)]
pub struct SerializedAmount {
    pub value: String,
    pub currency: String,
}

#[derive(Serialize)]
pub struct TransactionResponse {
    pub transaction: SerializedDirective,
}

#[derive(Deserialize)]
pub struct CommitRequest {
    pub expense_account: String,
}

#[derive(Serialize)]
pub struct CommitResponse {
    pub ok: bool,
    pub remaining_count: usize,
}

pub async fn init_handler(State(state): State<AppState>) -> Result<Json<InitResponse>, StatusCode> {
    let inner = state.inner.lock().unwrap();

    let items: Vec<SerializedDirective> = inner
        .staging_items
        .iter()
        .enumerate()
        .map(|(index, directive)| serialize_directive(index, directive))
        .collect();

    tracing::info!("Sending {} staging items", items.len());

    Ok(Json(InitResponse {
        items,
        current_index: 0,
        available_accounts: inner.available_accounts.iter().cloned().collect(),
    }))
}

pub async fn get_transaction(
    State(state): State<AppState>,
    Path(index): Path<usize>,
) -> Result<Json<TransactionResponse>, StatusCode> {
    let inner = state.inner.lock().unwrap();

    if index >= inner.staging_items.len() {
        return Err(StatusCode::NOT_FOUND);
    }

    let directive = &inner.staging_items[index];

    Ok(Json(TransactionResponse {
        transaction: serialize_directive(index, directive),
    }))
}

pub async fn commit_transaction(
    State(state): State<AppState>,
    Path(index): Path<usize>,
    Json(payload): Json<CommitRequest>,
) -> Result<Json<CommitResponse>, Response> {
    let mut inner = state.inner.lock().unwrap();

    if index >= inner.staging_items.len() {
        return Err(StatusCode::NOT_FOUND.into_response());
    }

    let expense_account = payload.expense_account;
    let directive = &inner.staging_items[index];

    // Use library function to commit transaction
    let journal_path = &inner.reconcile_config.journal_paths[0];
    beancount_staging::commit_transaction(directive, &expense_account, journal_path).map_err(
        |e| {
            tracing::error!("Failed to commit transaction {}: {}", index, e);
            ErrorResponse {
                error: format!("Failed to commit: {}", e),
            }
            .into_response()
        },
    )?;

    tracing::info!(
        "Committed transaction {} with expense account '{}'",
        index,
        expense_account
    );

    // Remove from staging items
    inner.staging_items.remove(index);

    let remaining_count = inner.staging_items.len();

    Ok(Json(CommitResponse {
        ok: true,
        remaining_count,
    }))
}

pub async fn file_changes_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let subscriber_count = state.file_change_tx.receiver_count();
    tracing::info!("New SSE connection. Total subscribers: {subscriber_count}",);

    let rx = state.file_change_tx.subscribe();
    let stream = BroadcastStream::new(rx).map(|_| Ok(Event::default().data("reload")));

    Sse::new(stream).keep_alive(KeepAlive::default())
}
