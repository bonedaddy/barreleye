use console::{style, Emoji};
use std::process;

use crate::{AppError, Warnings};

static SETUP: Emoji<'_, '_> = Emoji("💾  ", "");
static MIGRATIONS: Emoji<'_, '_> = Emoji("🚐  ", "");
static NETWORKS: Emoji<'_, '_> = Emoji("📢  ", "");
static READY: Emoji<'_, '_> = Emoji("🟢  ", "");
static QUIT: Emoji<'_, '_> = Emoji("🛑  ", "");

pub enum ReadyType {
	All(String),
	Server(String),
	Indexer,
}

pub enum Step {
	Setup,
	Migrations,
	Networks,
	Ready(ReadyType, Warnings),
}

pub async fn show(step: Step) {
	let out = |step, emoji, text| {
		println!("{} {}{}", style(format!("[{step}/4]")).bold().dim(), emoji, text,)
	};

	match step {
		Step::Setup => out(1, SETUP, "Checking setup…"),
		Step::Migrations => out(2, MIGRATIONS, "Running migrations…"),
		Step::Networks => out(3, NETWORKS, "Pinging networks…"),
		Step::Ready(ready_type, warnings) => {
			out(
				4,
				READY,
				&match ready_type {
					ReadyType::All(addr) => format!("Indexing & listening on {addr}…\n"),
					ReadyType::Server(addr) => format!("Listening on {addr}…\n"),
					ReadyType::Indexer => "Indexing…\n".to_string(),
				},
			);

			if !warnings.is_empty() {
				println!(
					"{}\n{}\n",
					style("Warnings:").yellow().bold(),
					warnings
						.iter()
						.map(|v| format!("{} {v}", style("↳").dim().bold()))
						.collect::<Warnings>()
						.join("\n"),
				);
			}
		}
	}
}

pub fn quit(app_error: AppError) {
	println!("{} {}Shutting down…\n\n› {}", style("[err]").bold().dim(), QUIT, app_error,);

	process::exit(match app_error {
		AppError::SignalHandler => exitcode::OSERR,
		AppError::ServerStartup { .. } => exitcode::OSERR,
		AppError::InvalidLeaderConfigs => exitcode::CONFIG,
		AppError::InvalidSetting { .. } => exitcode::CONFIG,
		_ => exitcode::UNAVAILABLE,
	});
}
