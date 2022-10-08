pub const DRIVER_SQLITE: &str = "sqlite";
pub const DRIVER_POSTGRES: &str = "postgres";
pub const DRIVER_MYSQL: &str = "mysql";

pub static PREFIX_SANCTIONED_ADDRESS_ID: &str = "san_adr";

pub static DEFAULT_SETTINGS_FILENAME: &str = "settings.toml";
pub static DEFAULT_SETTINGS_CONTENT: &str = r#"
[server]
ip_v4 = "0.0.0.0"
ip_v6 = "" # "::"
port = 22773

[database]
driver = "sqlite"
name = "barreleye_insights"
min_connections = 5
max_connections = 100
connect_timeout = 8
idle_timeout = 8
max_lifetime = 8

[database.sqlite]
url = "sqlite://data.db?mode=rwc"

[database.postgres]
url = "" # eg: "postgres://user:password@localhost:5432"

[database.mysql]
url = "" # eg: "mysql://user:password@localhost:3306"
"#;
