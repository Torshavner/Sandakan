use sandakan::application::ports::ConversationRepository;
use sandakan::domain::{Conversation, Message, MessageRole};

use crate::helpers::TestPostgres;

#[tokio::test]
async fn given_new_conversation_when_creating_and_retrieving_then_conversation_is_persisted() {
    let test_pg = TestPostgres::new().await;

    let conversation = Conversation::new(Some("Test Conversation".to_string()));
    let conversation_id = conversation.id;

    test_pg
        .conversation_repository
        .create_conversation(&conversation)
        .await
        .expect("Failed to create conversation");

    let retrieved = test_pg
        .conversation_repository
        .get_conversation(conversation_id)
        .await
        .expect("Failed to retrieve conversation")
        .expect("Conversation not found");

    assert_eq!(retrieved.id, conversation.id);
    assert_eq!(retrieved.title, conversation.title);
    assert!(retrieved.messages.is_empty());
}

#[tokio::test]
async fn given_conversation_when_appending_messages_then_messages_are_stored_in_order() {
    let test_pg = TestPostgres::new().await;

    let conversation = Conversation::new(Some("Chat Session".to_string()));
    let conversation_id = conversation.id;

    test_pg
        .conversation_repository
        .create_conversation(&conversation)
        .await
        .expect("Failed to create conversation");

    let msg1 = Message::new(conversation_id, MessageRole::User, "Hello".to_string());
    let msg2 = Message::new(
        conversation_id,
        MessageRole::Assistant,
        "Hi there!".to_string(),
    );

    test_pg
        .conversation_repository
        .append_message(&msg1)
        .await
        .expect("Failed to append first message");

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    test_pg
        .conversation_repository
        .append_message(&msg2)
        .await
        .expect("Failed to append second message");

    let messages = test_pg
        .conversation_repository
        .get_messages(conversation_id, 10)
        .await
        .expect("Failed to get messages");

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].content, "Hello");
    assert_eq!(messages[0].role, MessageRole::User);
    assert_eq!(messages[1].content, "Hi there!");
    assert_eq!(messages[1].role, MessageRole::Assistant);
}

#[tokio::test]
async fn given_multiple_messages_when_getting_with_limit_then_returns_most_recent() {
    let test_pg = TestPostgres::new().await;

    let conversation = Conversation::new(None);
    let conversation_id = conversation.id;

    test_pg
        .conversation_repository
        .create_conversation(&conversation)
        .await
        .expect("Failed to create conversation");

    for i in 0..5 {
        let msg = Message::new(conversation_id, MessageRole::User, format!("Message {}", i));
        test_pg
            .conversation_repository
            .append_message(&msg)
            .await
            .expect("Failed to append message");
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    let messages = test_pg
        .conversation_repository
        .get_messages(conversation_id, 3)
        .await
        .expect("Failed to get messages");

    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0].content, "Message 2");
    assert_eq!(messages[1].content, "Message 3");
    assert_eq!(messages[2].content, "Message 4");
}

#[tokio::test]
async fn given_nonexistent_conversation_id_when_retrieving_then_returns_none() {
    let test_pg = TestPostgres::new().await;

    let nonexistent_id = sandakan::domain::ConversationId::new();
    let result = test_pg
        .conversation_repository
        .get_conversation(nonexistent_id)
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}
