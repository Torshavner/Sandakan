mod azure_store;
mod local_store;
mod mock_store;
mod store_factory;

pub use azure_store::AzureStagingStore;
pub use local_store::LocalStagingStore;
pub use mock_store::MockStagingStore;
pub use store_factory::StagingStoreFactory;
