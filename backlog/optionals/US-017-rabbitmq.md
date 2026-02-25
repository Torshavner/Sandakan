Title: Implement Abstract Broker Abstraction for Background Jobs

Epic: Background Job Processing
Story Points: (Assign based on your team's scale)

User Story

As a backend developer,
I want to implement our background job worker using an abstracted messaging interface,
So that we can use fast, in-memory Tokio channels for our current scheduled jobs, while ensuring we can seamlessly swap to RabbitMQ in the future without rewriting the core worker logic.

Context & Business Value

Currently, we only need to process locally scheduled jobs, making a full RabbitMQ deployment unnecessary overhead. However, as the application scales, we anticipate needing distributed background workers. By decoupling the transport layer (Tokio mpsc vs. RabbitMQ) from the business logic now, we prevent a costly architectural rewrite later. This guarantees that all messages processed today are naturally serializable and network-ready for tomorrow.

Acceptance Criteria

Define Broker Errors: Create a custom BrokerError enum that standardizes errors across both local and network brokers (must include Publish, Receive, and Serialization error states).

Define Core Traits: Create asynchronous Publisher<T> and Subscriber<T> traits.

Enforce Serialization: The traits must enforce T: Serialize + Send + Sync + 'static for publishers and T: DeserializeOwned + Send + Sync + 'static for subscribers to guarantee future RabbitMQ compatibility.

Implement Local Adapters: Create LocalPublisher and LocalSubscriber structs that implement the traits by wrapping tokio::sync::mpsc channels.

Decouple the Worker: The main background worker function must take the Subscriber<T> trait as a generic parameter, containing zero references to Tokio or RabbitMQ specific logic.

Error Handling: If a channel closes or a message fails to deserialize, the worker should log the error appropriately and either continue polling or shut down gracefully.

Technical Implementation Notes

Use thiserror for the BrokerError definitions.

In the future RabbitMQ (lapin) implementation, the receive method of the Subscriber trait will be responsible for deserializing the byte payload via serde_json and sending the explicit .ack() back to the broker. The worker business logic should not know about acks.

Even though the Tokio implementation does not need to serialize data to pass it across threads, enforcing the serde trait bounds now prevents developers from accidentally passing unserializable data (like raw memory pointers or database connection pools) into the job queue.