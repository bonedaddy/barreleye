use console::{style, Emoji};

pub fn show() {
	println!(
		"› {}{}\n› https://barreleye.com/insights\n",
		style("Barreleye Insights").bold(),
		Emoji(" 🪪", ""),
	);
}
