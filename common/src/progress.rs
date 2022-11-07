use console::{style, Emoji};
use std::process;

use crate::AppError;

static SETUP: Emoji<'_, '_> = Emoji("💾  ", "");
static MIGRATIONS: Emoji<'_, '_> = Emoji("🚐  ", "");
static NETWORKS: Emoji<'_, '_> = Emoji("📢  ", "");
static READY: Emoji<'_, '_> = Emoji("🟢  ", "");
static QUIT: Emoji<'_, '_> = Emoji("🛑  ", "");

pub enum Step {
	Setup,
	Migrations,
	Networks,
	Ready(String),
}

pub async fn show(step: Step, is_watcher: bool) {
	let total_steps = if is_watcher { 4 } else { 3 };

	match step {
		Step::Setup => {
			println!(
				"{} {}Checking setup…",
				style(format!("[1/{total_steps}]")).bold().dim(),
				SETUP
			);
		}
		Step::Migrations => {
			println!(
				"{} {}Running migrations…",
				style(format!("[2/{total_steps}]")).bold().dim(),
				MIGRATIONS
			);
		}
		Step::Networks => {
			println!(
				"{} {}Pinging networks…",
				style(format!("[3/{total_steps}]")).bold().dim(),
				NETWORKS
			);
		}
		Step::Ready(addr) => {
			println!(
				"{} {}Listening on {}…",
				style(format!("[{total_steps}/{total_steps}]")).bold().dim(),
				READY,
				addr,
			);
		}
	}
}

pub fn quit(app_error: AppError) {
	println!(
		"{} {}Shutting down…\n\n› {}",
		style("[err]").bold().dim(),
		QUIT,
		app_error,
	);

	process::exit(match app_error {
		AppError::SignalHandler => exitcode::OSERR,
		AppError::InvalidSetting { .. } => exitcode::CONFIG,
		_ => exitcode::UNAVAILABLE,
	});
}
