use axum::{
	error_handling::HandleErrorLayer,
	extract::State,
	http::{header, Method, Request, StatusCode, Uri},
	middleware::{self, Next},
	response::Response,
	BoxError, Router, Server as AxumServer,
};
use console::style;
use eyre::{Report, Result};
use hyper::server::{accept::Accept, conn::AddrIncoming};
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

use crate::errors::ServerError;
use barreleye_common::{models::ApiKey, progress, progress::Step, AppError, AppState};

mod errors;
mod handlers;

pub type ServerResult<T> = Result<T, ServerError>;

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
		if let Poll::Ready(Some(value)) = Pin::new(&mut self.a).poll_accept(cx) {
			return Poll::Ready(Some(value));
		}

		if let Poll::Ready(Some(value)) = Pin::new(&mut self.b).poll_accept(cx) {
			return Poll::Ready(Some(value));
		}

		Poll::Pending
	}
}

pub struct Server {
	app_state: Arc<AppState>,
}

impl Server {
	pub fn new(app_state: Arc<AppState>) -> Self {
		Self { app_state }
	}

	async fn auth<B>(
		State(app): State<Arc<AppState>>,
		req: Request<B>,
		next: Next<B>,
	) -> ServerResult<Response> {
		for public_endpoint in vec!["/v0/assets", "/v0/upstream", "/v0/related"].iter() {
			if req.uri().to_string().starts_with(public_endpoint) {
				return Ok(next.run(req).await);
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

		let api_key = Uuid::parse_str(&token).map_err(|_| ServerError::Unauthorized)?;

		match ApiKey::get_by_uuid(&app.db, &api_key).await.map_err(|_| ServerError::Unauthorized)? {
			Some(api_key) if api_key.is_active => Ok(next.run(req).await),
			_ => Err(ServerError::Unauthorized),
		}
	}

	pub async fn start(&self) -> Result<()> {
		let settings = self.app_state.settings.clone();

		async fn handle_404() -> ServerResult<StatusCode> {
			Err(ServerError::NotFound)
		}

		async fn handle_timeout_error(
			method: Method,
			uri: Uri,
			_err: BoxError,
		) -> ServerResult<StatusCode> {
			Err(ServerError::Internal { error: Report::msg(format!("`{method} {uri}` timed out")) })
		}

		let app = Router::new()
			.nest("/", handlers::get_routes())
			.route_layer(middleware::from_fn_with_state(self.app_state.clone(), Self::auth))
			.fallback(handle_404)
			.layer(
				ServiceBuilder::new()
					.layer(HandleErrorLayer::new(handle_timeout_error))
					.timeout(Duration::from_secs(30)),
			)
			.with_state(self.app_state.clone());

		let ipv4 = SocketAddr::new(settings.server.ip_v4.parse()?, settings.server.port);

		let show_progress = |addr: &str| {
			progress::show(match self.app_state.is_indexer && self.app_state.is_server {
				true => Step::Ready(addr.to_string()),
				_ => Step::ServerReady(addr.to_string()),
			})
		};

		if settings.server.ip_v6.is_empty() {
			show_progress(&style(ipv4).bold().to_string()).await;

			match AxumServer::try_bind(&ipv4) {
				Err(e) => progress::quit(AppError::ServerStartup {
					url: ipv4.to_string(),
					error: e.message().to_string(),
				}),
				Ok(server) => {
					self.app_state.set_is_ready();
					server
						.serve(app.into_make_service())
						.with_graceful_shutdown(Self::shutdown_signal())
						.await?
				}
			}
		} else {
			let ipv6 = SocketAddr::new(settings.server.ip_v6.parse()?, settings.server.port);

			match (AddrIncoming::bind(&ipv4), AddrIncoming::bind(&ipv6)) {
				(Err(e), _) => progress::quit(AppError::ServerStartup {
					url: ipv4.to_string(),
					error: e.message().to_string(),
				}),
				(_, Err(e)) => progress::quit(AppError::ServerStartup {
					url: ipv6.to_string(),
					error: e.message().to_string(),
				}),
				(Ok(a), Ok(b)) => {
					show_progress(&format!("{} & {}", style(ipv4).bold(), style(ipv6).bold()))
						.await;

					self.app_state.set_is_ready();
					AxumServer::builder(CombinedIncoming { a, b })
						.serve(app.into_make_service())
						.with_graceful_shutdown(Self::shutdown_signal())
						.await?;
				}
			}
		}

		Ok(())
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
	}
}
