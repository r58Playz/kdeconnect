use crate::device::DeviceConfig;

use async_trait::async_trait;
use serde_json as json;
use std::{io, path::PathBuf};
use tokio::{fs::File, io::AsyncWriteExt};

#[async_trait]
pub trait ConfigProvider {
    async fn store_server_keypair(&self, cert: &[u8]) -> io::Result<()>;
    async fn retrieve_server_keypair(&self) -> io::Result<Vec<u8>>;
    async fn store_server_cert(&self, cert: &[u8]) -> io::Result<()>;
    async fn retrieve_server_cert(&self) -> io::Result<Vec<u8>>;
    async fn store_device_config(&self, config: &DeviceConfig) -> io::Result<()>;
    async fn retrieve_device_config(&self, id: &str) -> io::Result<DeviceConfig>;
}

pub struct FsConfig {
    pub path: PathBuf,
    pub cert_path: PathBuf,
    pub keypair_path: PathBuf,
}

impl FsConfig {
    pub fn new(path: PathBuf, cert_file_name: String, keypair_file_name: String) -> Self {
        Self {
            cert_path: path.join(cert_file_name),
            keypair_path: path.join(keypair_file_name),
            path,
        }
    }
}

#[async_trait]
impl ConfigProvider for FsConfig {
    async fn store_server_keypair(&self, cert: &[u8]) -> io::Result<()> {
        File::create(self.keypair_path.clone())
            .await?
            .write_all(cert)
            .await
    }

    async fn retrieve_server_keypair(&self) -> io::Result<Vec<u8>> {
        tokio::fs::read(self.keypair_path.clone()).await
    }

    async fn store_server_cert(&self, cert: &[u8]) -> io::Result<()> {
        File::create(self.cert_path.clone())
            .await?
            .write_all(cert)
            .await
    }

    async fn retrieve_server_cert(&self) -> io::Result<Vec<u8>> {
        tokio::fs::read(self.cert_path.clone()).await
    }

    async fn store_device_config(&self, config: &DeviceConfig) -> io::Result<()> {
        File::create(self.path.join(&config.id))
            .await?
            .write_all(&json::to_vec(config).map_err(io::Error::other)?)
            .await
    }

    async fn retrieve_device_config(&self, id: &str) -> io::Result<DeviceConfig> {
        json::from_slice(&tokio::fs::read(self.path.join(id)).await?).map_err(io::Error::other)
    }
}
