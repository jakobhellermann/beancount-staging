use beancount_staging::reconcile::StagingSource;
use beancount_staging_web::ListenerType;

#[tokio::test]
async fn test_api_workflow() {
    // Create temporary test files
    let temp_dir = std::env::temp_dir().join(format!("beancount-test-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let journal_path = temp_dir.join("journal.beancount");
    let staging_path = temp_dir.join("staging.beancount");

    // Write test journal file
    std::fs::write(
        &journal_path,
        r#"
2024-01-01 open Assets:Checking
2024-01-01 open Expenses:Groceries

2024-01-15 * "Existing Transaction"
    Assets:Checking  -50.00 USD
    Expenses:Groceries
"#,
    )
    .unwrap();

    // Write test staging file
    std::fs::write(
        &staging_path,
        r#"
2024-01-15 * "Existing Transaction"
    Assets:Checking  -50.00 USD

2024-01-20 * "New Transaction"
    Assets:Checking  -25.00 USD

2024-01-21 * "Another Transaction"
    Assets:Checking  -30.00 USD
"#,
    )
    .unwrap();

    let journal = vec![journal_path];
    let staging = vec![staging_path];

    // technically this can race but it seems fast enough for now
    tokio::spawn(async move {
        beancount_staging_web::run(
            journal,
            StagingSource::Files(staging),
            ListenerType::Tcp(8081),
        )
        .await
        .ok();
    });

    let client = reqwest::Client::new();
    let base = "http://localhost:8081";

    // Test 1: Init endpoint returns data
    let init: serde_json::Value = client
        .get(format!("{}/api/init", base))
        .send()
        .await
        .expect("init request failed")
        .json()
        .await
        .expect("init json parse failed");

    // Should have exactly 2 staging items (the two "New Transaction" and "Another Transaction")
    let items = init["items"].as_array().expect("items should be array");
    assert_eq!(items.len(), 2, "should have exactly 2 staging items");

    // Check available accounts
    let accounts = init["available_accounts"]
        .as_array()
        .expect("available_accounts should be array");
    assert!(accounts.contains(&serde_json::json!("Assets:Checking")));
    assert!(accounts.contains(&serde_json::json!("Expenses:Groceries")));

    // Get the ID of the first transaction
    let first_txn_id = items[0]["id"].as_str().expect("id should be a string");

    // Test 2: Get first transaction
    let txn: serde_json::Value = client
        .get(format!("{}/api/transaction/{}", base, first_txn_id))
        .send()
        .await
        .expect("get transaction failed")
        .json()
        .await
        .expect("transaction json parse failed");

    // Check structured transaction data (should be the "New Transaction")
    let txn_data = &txn["transaction"];

    // Check type
    assert_eq!(txn_data["type"], "transaction", "should be a transaction");

    // Check date
    assert_eq!(txn_data["date"], "2024-01-20", "should have correct date");

    // Check narration
    assert_eq!(
        txn_data["narration"], "New Transaction",
        "should have correct narration"
    );

    // Check postings
    let postings = txn_data["postings"]
        .as_array()
        .expect("postings should be array");
    assert_eq!(postings.len(), 1, "should have 1 posting");

    let posting = &postings[0];

    // Check account
    assert_eq!(
        posting["account"], "Assets:Checking",
        "should have correct account"
    );

    // Check amount
    let amount = &posting["amount"];
    assert!(!amount.is_null(), "posting should have an amount");
    assert_eq!(
        amount["value"], "-25.00",
        "should have correct amount value"
    );
    assert_eq!(amount["currency"], "USD", "should have correct currency");

    // Test 3: Commit transaction (should fail without expense_account)
    let commit_result = client
        .post(format!("{}/api/transaction/{}/commit", base, first_txn_id))
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("commit request failed");

    assert_eq!(
        commit_result.status().as_u16(),
        422,
        "commit without expense_account should return 422"
    );

    // Test 4: Commit transaction successfully
    let commit_response: serde_json::Value = client
        .post(format!("{}/api/transaction/{}/commit", base, first_txn_id))
        .json(&serde_json::json!({
            "account": "Expenses:Groceries"
        }))
        .send()
        .await
        .expect("commit request failed")
        .json()
        .await
        .expect("commit json parse failed");

    assert_eq!(commit_response["ok"], true, "commit should succeed");
    assert_eq!(
        commit_response["remaining_count"], 1,
        "should have 1 remaining transaction"
    );

    // Test 5: Verify transaction was removed from staging
    let init2: serde_json::Value = client
        .get(format!("{}/api/init", base))
        .send()
        .await
        .expect("init request failed")
        .json()
        .await
        .expect("init json parse failed");

    let items2 = init2["items"].as_array().expect("items should be array");
    assert_eq!(items2.len(), 1, "should have 1 staging item left");

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_file_watching_detects_changes() {
    // Create temporary test files
    let temp_dir =
        std::env::temp_dir().join(format!("beancount-watch-test-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let journal_path = temp_dir.join("journal.beancount");
    let staging_path = temp_dir.join("staging.beancount");

    // Write test journal file
    std::fs::write(
        &journal_path,
        r#"
2024-01-01 open Assets:Checking
2024-01-01 open Expenses:Groceries
"#,
    )
    .unwrap();

    // Write test staging file with one transaction
    std::fs::write(
        &staging_path,
        r#"
2024-01-20 * "Initial Transaction"
    Assets:Checking  -25.00 USD
"#,
    )
    .unwrap();

    let journal = vec![journal_path.clone()];
    let staging = vec![staging_path.clone()];

    // Start the server
    tokio::spawn(async move {
        beancount_staging_web::run(
            journal,
            StagingSource::Files(staging),
            ListenerType::Tcp(8082),
        )
        .await
        .ok();
    });

    let client = reqwest::Client::new();
    let base = "http://localhost:8082";

    // Wait for server to start
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Test 1: Verify initial state has 1 transaction
    let init: serde_json::Value = client
        .get(format!("{}/api/init", base))
        .send()
        .await
        .expect("init request failed")
        .json()
        .await
        .expect("init json parse failed");

    let items = init["items"].as_array().expect("items should be array");
    assert_eq!(
        items.len(),
        1,
        "should have exactly 1 staging item initially"
    );

    // Test 2: Connect to SSE endpoint to listen for file changes
    let sse_request = client
        .get(format!("{}/api/file-changes", base))
        .send()
        .await
        .expect("SSE connection failed");

    // Start listening for SSE events in the background
    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);
    tokio::spawn(async move {
        use futures::StreamExt;
        let mut stream = sse_request.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            if let Ok(bytes) = chunk_result {
                let data = String::from_utf8_lossy(&bytes);
                // SSE events contain "data: reload"
                if data.contains("reload") {
                    let _ = tx.send(()).await;
                    break;
                }
            }
        }
    });

    // Wait a bit to ensure SSE connection is established
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Test 3: Modify the staging file by adding a new transaction
    std::fs::write(
        &staging_path,
        r#"
2024-01-20 * "Initial Transaction"
    Assets:Checking  -25.00 USD

2024-01-25 * "New Transaction After Modification"
    Assets:Checking  -30.00 USD
"#,
    )
    .unwrap();

    // Test 4: Wait for file change event (with timeout)
    let received_event = tokio::time::timeout(tokio::time::Duration::from_secs(2), rx.recv()).await;

    assert!(
        received_event.is_ok(),
        "Should receive SSE file change event within 2 seconds after file modification"
    );

    // Give the server time to reload state after file change
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Test 5: Verify the state was reloaded with the new transaction
    let init2: serde_json::Value = client
        .get(format!("{}/api/init", base))
        .send()
        .await
        .expect("init request failed after file change")
        .json()
        .await
        .expect("init json parse failed after file change");

    let items2 = init2["items"].as_array().expect("items should be array");
    assert_eq!(
        items2.len(),
        2,
        "should have 2 staging items after file modification"
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}
