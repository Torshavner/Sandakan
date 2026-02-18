mod azure_store;
mod local_store;
mod store_factory;
mod mock_store;

pub use azure_store::AzureStagingStore;
pub use local_store::LocalStagingStore;
pub use store_factory::StagingStoreFactory;
pub use mock_store::MockStagingStore;