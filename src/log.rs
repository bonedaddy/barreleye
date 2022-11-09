use chrono::Local;
use eyre::Result;
use fern::colors::{Color, ColoredLevelConfig};
use log::LevelFilter;

pub fn setup() -> Result<()> {
	color_eyre::install()?;

	let colors_line = ColoredLevelConfig::new()
		.error(Color::Red)
		.warn(Color::Yellow)
		.info(Color::White)
		.debug(Color::White)
		.trace(Color::BrightBlack);

	fern::Dispatch::new()
		.format(move |out, message, record| {
			if message.to_string() != "" {
				out.finish(format_args!(
					"{color_line}{date} · {message}\x1B[0m",
					color_line = format_args!(
						"\x1B[{}m",
						colors_line.get_color(&record.level()).to_fg_str()
					),
					date = Local::now().format("%Y-%m-%d %H:%M:%S"),
					message = message,
				));
			} else {
				out.finish(format_args!(""));
			}
		})
		.level(LevelFilter::Warn)
		.level_for("axum", LevelFilter::Info)
		.level_for("barreleye_chain", LevelFilter::Info)
		.level_for("barreleye_common", LevelFilter::Info)
		.level_for("barreleye_server", LevelFilter::Info)
		.chain(std::io::stdout())
		.apply()?;

	Ok(())
}
