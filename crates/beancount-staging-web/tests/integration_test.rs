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
        beancount_staging_web::run(journal, staging, 8081)
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

    // Test 2: Get first transaction
    let txn: serde_json::Value = client
        .get(format!("{}/api/transaction/0", base))
        .send()
        .await
        .expect("get transaction failed")
        .json()
        .await
        .expect("transaction json parse failed");

    // Should be the "New Transaction"
    let content = txn["transaction"]["content"]
        .as_str()
        .expect("content should be string");
    assert!(content.contains("2024-01-20"), "should contain date");
    assert!(
        content.contains("New Transaction"),
        "should contain narration"
    );
    assert!(
        content.contains("Assets:Checking"),
        "should contain account"
    );
    assert!(content.contains("-25.00 USD"), "should contain amount");

    // Test 3: Commit transaction (should fail without expense_account)
    let commit_result = client
        .post(format!("{}/api/transaction/0/commit", base))
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
        .post(format!("{}/api/transaction/0/commit", base))
        .json(&serde_json::json!({"expense_account": "Expenses:Groceries"}))
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
