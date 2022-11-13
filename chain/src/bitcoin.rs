use async_trait::async_trait;
use bitcoincore_rpc::{Auth, Client, RpcApi};
use eyre::{bail, Result};
use indicatif::ProgressBar;
use std::sync::Arc;
use url::Url;

use crate::ChainTrait;
use barreleye_common::{models::Network, utils, AppState};

pub struct Bitcoin {
	app_state: Arc<AppState>,
	network: Network,
	rpc: Option<String>,
	client: Arc<Client>,
}

impl Bitcoin {
	pub async fn new(
		app_state: Arc<AppState>,
		network: Network,
		pb: Option<&ProgressBar>,
	) -> Result<Self> {
		let mut rpc: Option<String> = None;
		let mut maybe_client: Option<Client> = None;

		let mut rpc_endpoints = vec![];

		let (message_trying, message_failed) = if network.rpc.is_empty() {
			rpc_endpoints =
				serde_json::from_value(network.rpc_bootstraps.clone())?;
			(
				"trying rpc endpoints…".to_string(),
				"Could not connect to any RPC endpoint.".to_string(),
			)
		} else {
			rpc_endpoints.push(network.rpc.clone());
			(
				"connecting to rpc…".to_string(),
				format!(
					"Could not connect to RPC endpoint @ `{}`.",
					utils::with_masked_auth(&network.rpc)
				),
			)
		};

		if let Some(pb) = pb {
			pb.set_message(message_trying);
		}

		for url in rpc_endpoints.into_iter() {
			if let Ok(u) = Url::parse(&url) {
				let auth = match (u.username(), u.password()) {
					(username, Some(password)) => Auth::UserPass(
						username.to_string(),
						password.to_string(),
					),
					_ => Auth::None,
				};

				if let Ok(client) = Client::new(&url, auth) {
					if client.get_blockchain_info().is_ok() {
						rpc = Some(url);
						maybe_client = Some(client);
					}
				}
			}
		}

		if maybe_client.is_none() {
			if let Some(pb) = pb {
				pb.abandon();
			}

			bail!(format!("{}: {}", network.name, message_failed));
		}

		Ok(Self {
			app_state,
			network,
			rpc,
			client: Arc::new(maybe_client.unwrap()),
		})
	}
}

#[async_trait]
impl ChainTrait for Bitcoin {
	fn get_network(&self) -> Network {
		self.network.clone()
	}

	fn get_rpc(&self) -> Option<String> {
		self.rpc.clone()
	}

	async fn process_blocks(&self) -> Result<()> {
		// println!("processing blocks at {}…", self.network.id); // @TODO
		Ok(())
	}
}
