use clap::{arg, command, value_parser};
use eyre::Result;
use std::env;

use barreleye_common::Env;

mod banner;
mod log;

fn main() -> Result<()> {
	log::setup()?;

	let matches = command!()
		.author("Barreleye")
		.version(env!("CARGO_PKG_VERSION"))
		.propagate_version(true)
		.arg(
			arg!(-e --env <ENV> "Network types to connect to")
				.value_parser(value_parser!(Env)),
		)
		.arg(arg!(--indexer "Run only indexer, without the server"))
		.arg(arg!(--server "Run only server, without the indexer"))
		.arg(arg!(-p --plain "No ASCII banner"))
		.get_matches();

	let env: Env = *matches.get_one("env").unwrap_or(&Env::Mainnet);
	let skip_ascii: bool = *matches.get_one("plain").unwrap_or(&false);

	let (is_indexer, is_server) = match (
		*matches.get_one("indexer").unwrap_or(&false),
		*matches.get_one("server").unwrap_or(&false),
	) {
		(true, _) => (true, false),
		(_, true) => (false, true),
		_ => (true, true),
	};

	banner::show(env, is_indexer, is_server, skip_ascii)?;
	barreleye_server::start(env, is_indexer, is_server)?;

	Ok(())
}
