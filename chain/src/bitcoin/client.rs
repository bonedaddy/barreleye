use bitcoincore_rpc::{
	jsonrpc::Error as JsonRpcError, Client, Error as BitcoinRpcError, Result as BitcoinRpcResult,
	RpcApi,
};
use serde::de::Deserialize;
use serde_json::Value as JsonValue;
use std::{thread::sleep, time::Duration};

// source: `https://github.com/bitcoin/bitcoin/blob/master/src/rpc/protocol.h`
const RPC_IN_WARMUP: i32 = -28;

const RETRY_ATTEMPTS: u8 = 10;
const RPC_TIMEOUT: u64 = 1_000;

pub struct RetryClient {
	client: Client,
}

impl RetryClient {
	pub fn new(client: Client) -> Self {
		Self { client }
	}
}

impl RpcApi for RetryClient {
	fn call<T: for<'a> Deserialize<'a>>(
		&self,
		cmd: &str,
		args: &[JsonValue],
	) -> BitcoinRpcResult<T> {
		for _ in 0..RETRY_ATTEMPTS {
			match self.client.call(cmd, args) {
				Ok(ret) => return Ok(ret),
				Err(BitcoinRpcError::JsonRpc(JsonRpcError::Transport(e))) => {
					// @TODO until pattern matching for boxed types is in stable,
					// doing this hacky thing
					if e.to_string().contains("timed out") {
						sleep(Duration::from_millis(RPC_TIMEOUT));
						continue;
					}

					return Err(BitcoinRpcError::JsonRpc(JsonRpcError::Transport(e)));
				}
				Err(BitcoinRpcError::JsonRpc(JsonRpcError::Rpc(ref e)))
					if e.code == RPC_IN_WARMUP =>
				{
					sleep(Duration::from_millis(RPC_TIMEOUT));
					continue;
				}
				Err(e) => return Err(e),
			}
		}

		self.client.call(cmd, args)
	}
}
