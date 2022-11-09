use axum::{
	error_handling::HandleErrorLayer,
	extract::State,
	http::{header, Method, Request, StatusCode, Uri},
	middleware::{self, Next},
	response::Response,
	BoxError, Router, Server,
};
use console::style;
use eyre::{bail, Report, Result};
use hyper::server::{accept::Accept, conn::AddrIncoming};
use log::info;
use sea_orm::DatabaseConnection;
use signal::unix::SignalKind;
use std::{
	net::SocketAddr,
	pin::Pin,
	sync::Arc,
	task::{Context, Poll},
	time::Duration,
};
use tokio::signal;
use tower::ServiceBuilder;
use uuid::Uuid;

use crate::{errors::ServerError, handlers, ServerResult, ServerState};
use barreleye_chain::Networks;
use barreleye_common::{
	models::Account, progress, progress::Step, AppError, Clickhouse, Env,
	Settings,
};

async fn auth<B>(
	State(app): State<Arc<ServerState>>,
	mut req: Request<B>,
	next: Next<B>,
) -> ServerResult<Response> {
	let mut is_admin_key_required = true;
	for user_endpoint in vec!["/v0/insights"].iter() {
		if req.uri().to_string().starts_with(user_endpoint) {
			is_admin_key_required = false;
		}
	}

	let authorization = req
		.headers()
		.get(header::AUTHORIZATION)
		.ok_or(ServerError::Unauthorized)?
		.to_str()
		.map_err(|_| ServerError::Unauthorized)?;

	let token = match authorization.split_once(' ') {
		Some((name, contents)) if name == "Bearer" => contents.to_string(),
		_ => return Err(ServerError::Unauthorized),
	};

	let api_key =
		Uuid::parse_str(&token).map_err(|_| ServerError::Unauthorized)?;

	if let Some(account) =
		Account::get_by_api_key(&app.db, api_key, is_admin_key_required)
			.await
			.map_err(|_| ServerError::Unauthorized)?
	{
		req.extensions_mut().insert(account);
		Ok(next.run(req).await)
	} else {
		Err(ServerError::Unauthorized)
	}
}

pub async fn start(
	settings: Arc<Settings>,
	warehouse: Arc<Clickhouse>,
	db: Arc<DatabaseConnection>,
	networks: Option<Arc<Networks>>,
	env: Env,
	is_watcher: bool,
) -> Result<()> {
	let shared_state = Arc::new(ServerState::new(
		settings.clone(),
		warehouse,
		db,
		networks,
		env,
		is_watcher,
	));

	let app = wrap_router(
		Router::with_state(shared_state.clone())
			.merge(handlers::get_routes(shared_state.clone()))
			.route_layer(middleware::from_fn_with_state(shared_state, auth)),
	);

	let ipv4 =
		SocketAddr::new(settings.server.ip_v4.parse()?, settings.server.port);
	if settings.server.ip_v6.is_empty() {
		progress::show(Step::Ready(style(ipv4).bold().to_string()), is_watcher)
			.await;
		Server::bind(&ipv4)
			.serve(app.into_make_service())
			.with_graceful_shutdown(shutdown_signal())
			.await?;
	} else {
		let ipv6 = SocketAddr::new(
			settings.server.ip_v6.parse()?,
			settings.server.port,
		);

		let listeners = CombinedIncoming {
			a: AddrIncoming::bind(&ipv4)
				.or_else(|e| bail!(e.into_cause().unwrap()))?,
			b: AddrIncoming::bind(&ipv6)
				.or_else(|e| bail!(e.into_cause().unwrap()))?,
		};

		progress::show(
			Step::Ready(format!(
				"{} & {}",
				style(ipv4).bold(),
				style(ipv6).bold()
			)),
			is_watcher,
		)
		.await;

		Server::builder(listeners)
			.serve(app.into_make_service())
			.with_graceful_shutdown(shutdown_signal())
			.await?;
	}

	Ok(())
}

pub fn wrap_router(
	router: Router<Arc<ServerState>>,
) -> Router<Arc<ServerState>> {
	async fn handle_404() -> ServerResult<StatusCode> {
		Err(ServerError::NotFound)
	}

	async fn handle_timeout_error(
		method: Method,
		uri: Uri,
		_err: BoxError,
	) -> ServerResult<StatusCode> {
		Err(ServerError::Internal {
			error: Report::msg(format!("`{method} {uri}` timed out")),
		})
	}

	router.fallback(handle_404).layer(
		ServiceBuilder::new()
			.layer(HandleErrorLayer::new(handle_timeout_error))
			.timeout(Duration::from_secs(30)),
	)
}

struct CombinedIncoming {
	a: AddrIncoming,
	b: AddrIncoming,
}

impl Accept for CombinedIncoming {
	type Conn = <AddrIncoming as Accept>::Conn;
	type Error = <AddrIncoming as Accept>::Error;

	fn poll_accept(
		mut self: Pin<&mut Self>,
		cx: &mut Context<'_>,
	) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
		if let Poll::Ready(Some(value)) = Pin::new(&mut self.a).poll_accept(cx)
		{
			return Poll::Ready(Some(value));
		}

		if let Poll::Ready(Some(value)) = Pin::new(&mut self.b).poll_accept(cx)
		{
			return Poll::Ready(Some(value));
		}

		Poll::Pending
	}
}

async fn shutdown_signal() {
	let ctrl_c = async {
		if signal::ctrl_c().await.is_err() {
			progress::quit(AppError::SignalHandler);
		}
	};

	#[cfg(unix)]
	let terminate = async {
		match signal::unix::signal(SignalKind::terminate()) {
			Ok(mut signal) => {
				signal.recv().await;
			}
			_ => progress::quit(AppError::SignalHandler),
		};
	};

	#[cfg(not(unix))]
	let terminate = future::pending::<()>();

	tokio::select! {
		_ = ctrl_c => {},
		_ = terminate => {},
	}

	info!("");
	info!("SIGINT received; bye 👋");
}
