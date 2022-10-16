use clap::{arg, command, Command};
use color_eyre::eyre::WrapErr;
use eyre::Result;

mod banner;
mod log;

fn main() -> Result<()> {
	log::setup()?;

	let matches = command!()
		.author("Barreleye")
		.propagate_version(true)
		.subcommand_required(true)
		.arg_required_else_help(true)
		.subcommand(
			Command::new("scan").about("Start scanning blockchain data"),
		)
		.subcommand(
			Command::new("server").about("Start the insights server").arg(
				arg!(-p --plain "Don't bother displaying the ASCII banner"),
			),
		)
		.get_matches();

	match matches.subcommand() {
		Some(("scan", _)) => {
			banner::show(true)?;
			barreleye_scan::start().wrap_err("Could not start scan")?;
		}
		Some(("server", sub_matches)) => {
			let skip_ascii =
				*sub_matches.get_one::<bool>("plain").unwrap_or(&false);

			banner::show(skip_ascii)?;
			barreleye_server::start().wrap_err("Could not start server")?;
		}
		_ => unreachable!("No command found"),
	}

	Ok(())
}
