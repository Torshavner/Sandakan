mod azure_store;
mod local_store;
mod store_factory;

pub use azure_store::AzureStagingStore;
pub use local_store::LocalStagingStore;
pub use store_factory::StagingStoreFactory;
