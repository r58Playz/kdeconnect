use crate::{device::DeviceConfig, KdeConnectError, Result};

use async_trait::async_trait;
use serde_json as json;
use std::path::PathBuf;
use tokio::{
	fs::{create_dir_all, read_dir, File},
	io::AsyncWriteExt,
};

#[async_trait]
pub trait ConfigProvider {
	async fn store_server_keypair(&self, cert: &[u8]) -> Result<()>;
	async fn retrieve_server_keypair(&self) -> Result<Vec<u8>>;
	async fn store_server_cert(&self, cert: &[u8]) -> Result<()>;
	async fn retrieve_server_cert(&self) -> Result<Vec<u8>>;
	async fn store_device_config(&self, config: &DeviceConfig) -> Result<()>;
	async fn retrieve_device_config(&self, id: &str) -> Result<DeviceConfig>;
}

pub struct FsConfig {
	pub path: PathBuf,
	pub device_path: PathBuf,
	pub cert_path: PathBuf,
	pub keypair_path: PathBuf,
}

impl FsConfig {
	pub async fn new(
		path: PathBuf,
		cert_file_name: String,
		keypair_file_name: String,
		device_folder_name: String,
	) -> Result<Self> {
		let device_path = path.join(device_folder_name);
		create_dir_all(&path).await?;
		create_dir_all(&device_path).await?;
		Ok(Self {
			cert_path: path.join(cert_file_name),
			keypair_path: path.join(keypair_file_name),
			device_path,
			path,
		})
	}

	pub async fn retrieve_all_device_configs(&self) -> Result<Vec<DeviceConfig>> {
		let mut read_dir = read_dir(&self.device_path).await?;
		let mut out = Vec::new();
		while let Ok(Some(entry)) = read_dir.next_entry().await
			&& entry.metadata().await?.is_file()
		{
			out.push(
				self.retrieve_device_config(
					entry
						.file_name()
						.to_str()
						.ok_or(KdeConnectError::OsStringConversionError)?,
				)
				.await?,
			);
		}
		Ok(out)
	}
}

#[async_trait]
impl ConfigProvider for FsConfig {
	async fn store_server_keypair(&self, cert: &[u8]) -> Result<()> {
		Ok(File::create(&self.keypair_path)
			.await?
			.write_all(cert)
			.await?)
	}

	async fn retrieve_server_keypair(&self) -> Result<Vec<u8>> {
		Ok(tokio::fs::read(&self.keypair_path).await?)
	}

	async fn store_server_cert(&self, cert: &[u8]) -> Result<()> {
		Ok(File::create(&self.cert_path).await?.write_all(cert).await?)
	}

	async fn retrieve_server_cert(&self) -> Result<Vec<u8>> {
		Ok(tokio::fs::read(&self.cert_path).await?)
	}

	async fn store_device_config(&self, config: &DeviceConfig) -> Result<()> {
		Ok(File::create(self.device_path.join(&config.id))
			.await?
			.write_all(&json::to_vec(config)?)
			.await?)
	}

	async fn retrieve_device_config(&self, id: &str) -> Result<DeviceConfig> {
		Ok(json::from_slice(
			&tokio::fs::read(self.device_path.join(id)).await?,
		)?)
	}
}
