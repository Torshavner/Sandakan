use std::path::PathBuf;
use std::sync::Arc;

use crate::application::ports::{StagingStore, StagingStoreError};
use crate::presentation::config::{StorageProviderSetting, StorageSettings};

use super::azure_store::AzureStagingStore;
use super::local_store::LocalStagingStore;

pub struct StagingStoreFactory;

impl StagingStoreFactory {
    pub fn create(settings: &StorageSettings) -> Result<Arc<dyn StagingStore>, StagingStoreError> {
        match settings.provider {
            StorageProviderSetting::Local => {
                let path = PathBuf::from(&settings.local_path);
                let store = LocalStagingStore::new(path)?;
                Ok(Arc::new(store))
            }
            StorageProviderSetting::Azure => {
                let account = settings.azure_account.as_deref().ok_or_else(|| {
                    StagingStoreError::UploadFailed("azure_account required".into())
                })?;
                let key = settings.azure_access_key.as_deref().ok_or_else(|| {
                    StagingStoreError::UploadFailed("azure_access_key required".into())
                })?;
                let container = settings.azure_container.as_deref().ok_or_else(|| {
                    StagingStoreError::UploadFailed("azure_container required".into())
                })?;
                let store = AzureStagingStore::new(account, key, container)?;
                Ok(Arc::new(store))
            }
        }
    }
}
