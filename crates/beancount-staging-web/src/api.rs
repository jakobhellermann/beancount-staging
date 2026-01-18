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

use crate::state::{AppState, generate_directive_id};
use beancount_staging::Directive;

fn serialize_directive(directive: &Directive) -> SerializedDirective {
    use beancount_parser::DirectiveContent;

    let id = generate_directive_id(directive);

    let content = match &directive.content {
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

            SerializedDirectiveContent::Transaction(SerializedTransaction {
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
            })
        }
        DirectiveContent::Balance(bal) => SerializedDirectiveContent::Balance(SerializedBalance {
            date: directive.date.to_string(),
            account: bal.account.to_string(),
            amount: SerializedAmount {
                value: bal.amount.value.to_string(),
                currency: bal.amount.currency.to_string(),
            },
            tolerance: bal.tolerance.as_ref().map(|t| t.to_string()),
        }),
        other => todo!(
            "Directive type not yet supported for serialization: {:?}",
            other
        ),
    };

    SerializedDirective { id, content }
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
    pub id: String,
    #[serde(flatten)]
    pub content: SerializedDirectiveContent,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SerializedDirectiveContent {
    Transaction(SerializedTransaction),
    Balance(SerializedBalance),
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
pub struct SerializedBalance {
    pub date: String,
    pub account: String,
    pub amount: SerializedAmount,
    pub tolerance: Option<String>,
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
    pub predicted_account: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitRequest {
    pub account: String,
    pub payee: Option<String>,
    pub narration: Option<String>,
}

#[derive(Serialize)]
pub struct CommitResponse {
    pub ok: bool,
    pub remaining_count: usize,
}

pub async fn init_handler(State(state): State<AppState>) -> Result<Json<InitResponse>, StatusCode> {
    let inner = state.inner.lock().unwrap();

    // BTreeMap already maintains sorted order by key (date-hash)
    let items: Vec<SerializedDirective> = inner
        .staging_items
        .values()
        .map(serialize_directive)
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
    Path(id): Path<String>,
) -> Result<Json<TransactionResponse>, StatusCode> {
    let inner = state.lock().unwrap();

    let directive = inner.staging_items.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    let predicted_account = inner.predict(directive);

    Ok(Json(TransactionResponse {
        transaction: serialize_directive(directive),
        predicted_account: predicted_account.map(|account| account.to_string()),
    }))
}

pub async fn commit_transaction(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<CommitRequest>,
) -> Result<Json<CommitResponse>, Response> {
    let mut inner = state.lock().unwrap();

    let directive = inner
        .staging_items
        .get(&id)
        .ok_or(StatusCode::NOT_FOUND.into_response())?;

    // Use library function to commit transaction
    let journal_path = &inner.reconcile_config.journal_paths[0];
    beancount_staging::commit_transaction(
        directive,
        &payload.account,
        payload.payee.as_deref(),
        payload.narration.as_deref(),
        journal_path,
    )
    .map_err(|e| {
        tracing::error!("Failed to commit transaction {}: {}", id, e);
        ErrorResponse {
            error: format!("Failed to commit: {}", e),
        }
        .into_response()
    })?;

    tracing::info!("Committed transaction {} with patch: {:?}", id, payload);

    // Remove from staging items
    inner.staging_items.remove(&id);

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
