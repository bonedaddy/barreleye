use console::{style, Emoji};

static DATABASE: Emoji<'_, '_> = Emoji("💾  ", "");
static MIGRATIONS: Emoji<'_, '_> = Emoji("🚢  ", "");
static FETCHING: Emoji<'_, '_> = Emoji("📥  ", "");
static READY: Emoji<'_, '_> = Emoji("🟢  ", "");

pub enum Step {
	Database,
	Migrations,
	Fetching,
	Ready(String),
}

pub async fn show(step: Step) {
	match step {
		Step::Database => {
			println!(
				"{} {}Checking database…",
				style("[1/4]").bold().dim(),
				DATABASE
			);
		}
		Step::Migrations => {
			println!(
				"{} {}Running migrations…",
				style("[2/4]").bold().dim(),
				MIGRATIONS
			);
		}
		Step::Fetching => {
			println!(
				"{} {}Updating sanction lists…",
				style("[3/4]").bold().dim(),
				FETCHING
			);
		}
		Step::Ready(addr) => {
			println!(
				"{} {}Listening on {}…",
				style("[4/4]").bold().dim(),
				READY,
				addr,
			);
		}
	}
}
