use console::{style, Emoji};

static DATABASE: Emoji<'_, '_> = Emoji("💾  ", "");
static MIGRATIONS: Emoji<'_, '_> = Emoji("🚐  ", "");
static LISTENING: Emoji<'_, '_> = Emoji("🟢  ", "");
static SCANNING: Emoji<'_, '_> = Emoji("🔎  ", "");

pub enum Step {
	Database,
	Migrations,
	Listening(String),
	Scanning,
}

pub async fn show(step: Step) {
	match step {
		Step::Database => {
			println!(
				"{} {}Checking database…",
				style("[1/3]").bold().dim(),
				DATABASE
			);
		}
		Step::Migrations => {
			println!(
				"{} {}Running migrations…",
				style("[2/3]").bold().dim(),
				MIGRATIONS
			);
		}
		Step::Listening(addr) => {
			println!(
				"{} {}Listening on {}…",
				style("[3/3]").bold().dim(),
				LISTENING,
				addr,
			);
		}
		Step::Scanning => {
			println!("{} {}Scanning…", style("[3/3]").bold().dim(), SCANNING);
		}
	}
}
