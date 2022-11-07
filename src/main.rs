use clap::{arg, command, value_parser, Command};
use color_eyre::eyre::WrapErr;
use eyre::Result;

mod banner;
mod log;

use barreleye_common::Env;

fn main() -> Result<()> {
	log::setup()?;

	let matches = command!()
		.author("Barreleye")
		.propagate_version(true)
		.subcommand_required(true)
		.arg_required_else_help(true)
		.subcommand(
			Command::new("server")
				.about("Start the insights server")
				.arg(
					arg!(-w --watch "Run in watcher mode (push blockchain data to warehouse)"),
				)
				.arg(
					arg!(--env <ENV> "Network types to load")
						.value_parser(value_parser!(Env)),
				)
				.arg(arg!(-p --plain "No ASCII banner")),
		)
		.get_matches();

	match matches.subcommand() {
		Some(("server", opts)) => {
			let env: Env = *opts.get_one("env").unwrap_or(&Env::Mainnet);
			let is_watcher: bool = *opts.get_one("watch").unwrap_or(&false);
			let skip_ascii: bool = *opts.get_one("plain").unwrap_or(&false);

			banner::show(env, is_watcher, skip_ascii)?;
			barreleye_server::start(env, is_watcher)
				.wrap_err("Could not start the insights server")?;
		}
		_ => unreachable!("No command found"),
	}

	Ok(())
}
