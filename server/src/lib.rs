use axum::{Router, Server};
use console::style;
use eyre::{bail, Result};
use hyper::server::{accept::Accept, conn::AddrIncoming};
use log::info;
use signal::unix::SignalKind;
use std::{
	net::SocketAddr,
	pin::Pin,
	sync::Arc,
	task::{Context, Poll},
};
use tokio::signal;

use barreleye_common::{db, Settings, AppState};

mod handlers;

#[tokio::main]
pub async fn start() -> Result<()> {
	let settings = Settings::new()?;

	let shared_state = Arc::new(AppState { db: db::new().await? });
	let app = Router::with_state(shared_state.clone())
		.merge(handlers::get_routes(shared_state));

	let port = settings.server.port;
	let ip_v4 = SocketAddr::new(settings.server.ip_v4.parse()?, port);

	if settings.server.ip_v6.is_empty() {
		info!("Listening on {}…", style(ip_v4).bold());
		Server::bind(&ip_v4)
			.serve(app.into_make_service())
			.with_graceful_shutdown(shutdown_signal())
			.await?;
	} else {
		let ip_v6 = SocketAddr::new(settings.server.ip_v6.parse()?, port);

		let listeners = CombinedIncoming {
			a: AddrIncoming::bind(&ip_v4)
				.or_else(|e| bail!(e.into_cause().unwrap()))?,
			b: AddrIncoming::bind(&ip_v6)
				.or_else(|e| bail!(e.into_cause().unwrap()))?,
		};

		info!(
			"Listening on {} & {}…",
			style(ip_v4).bold(),
			style(ip_v6).bold()
		);
		Server::builder(listeners)
			.serve(app.into_make_service())
			.with_graceful_shutdown(shutdown_signal())
			.await?;
	}

	Ok(())
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
		signal::ctrl_c().await.expect("Failed to install Ctrl+C handler");
	};

	#[cfg(unix)]
	let terminate = async {
		signal::unix::signal(SignalKind::terminate())
			.expect("Failed to install signal handler")
			.recv()
			.await;
	};

	#[cfg(not(unix))]
	let terminate = future::pending::<()>();

	tokio::select! {
		_ = ctrl_c => {},
		_ = terminate => {},
	}

	info!("");
	info!("SIGINT received; shutting down 👋");
}
