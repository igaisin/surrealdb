use crate::api::conn::Connection;
use crate::api::conn::Method;
use crate::api::conn::Param;
use crate::api::conn::Route;
use crate::api::conn::Router;
#[allow(unused_imports)] // used by the DB engines
use crate::api::engine;
use crate::api::engine::any::Any;
#[cfg(feature = "protocol-http")]
use crate::api::engine::remote::http;
use crate::api::err::Error;
use crate::api::opt::Endpoint;
#[cfg(any(feature = "native-tls", feature = "rustls"))]
#[cfg(feature = "protocol-http")]
use crate::api::opt::Tls;
use crate::api::DbResponse;
#[allow(unused_imports)] // used by the DB engines
use crate::api::ExtraFeatures;
use crate::api::Result;
use crate::api::Surreal;
use crate::error::Db as DbError;
use flume::Receiver;
use once_cell::sync::OnceCell;
#[cfg(feature = "protocol-http")]
use reqwest::ClientBuilder;
use std::collections::HashSet;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::atomic::AtomicI64;
use std::sync::Arc;
#[cfg(feature = "protocol-ws")]
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
#[cfg(feature = "protocol-ws")]
#[cfg(any(feature = "native-tls", feature = "rustls"))]
use tokio_tungstenite::Connector;

impl crate::api::Connection for Any {}

impl Connection for Any {
	fn new(method: Method) -> Self {
		Self {
			method,
			id: 0,
		}
	}

	#[allow(unused_variables, unreachable_code, unused_mut)] // these are all used depending on feature
	fn connect(
		address: Endpoint,
		capacity: usize,
	) -> Pin<Box<dyn Future<Output = Result<Surreal<Self>>> + Send + Sync + 'static>> {
		Box::pin(async move {
			let (route_tx, route_rx) = match capacity {
				0 => flume::unbounded(),
				capacity => flume::bounded(capacity),
			};

			let (conn_tx, conn_rx) = flume::bounded::<Result<()>>(1);
			let mut features = HashSet::new();

			match address.endpoint.scheme() {
				"fdb" => {
					#[cfg(feature = "kv-fdb")]
					{
						features.insert(ExtraFeatures::Backup);
						engine::local::native::router(address, conn_tx, route_rx);
						conn_rx.into_recv_async().await??
					}

					#[cfg(not(feature = "kv-fdb"))]
					return Err(
						DbError::Ds("Cannot connect to the `foundationdb` storage engine as it is not enabled in this build of SurrealDB".to_owned()).into()
					);
				}

				"mem" => {
					#[cfg(feature = "kv-mem")]
					{
						features.insert(ExtraFeatures::Backup);
						engine::local::native::router(address, conn_tx, route_rx);
						conn_rx.into_recv_async().await??
					}

					#[cfg(not(feature = "kv-mem"))]
					return Err(
						DbError::Ds("Cannot connect to the `memory` storage engine as it is not enabled in this build of SurrealDB".to_owned()).into()
					);
				}

				"file" | "rocksdb" => {
					#[cfg(feature = "kv-rocksdb")]
					{
						features.insert(ExtraFeatures::Backup);
						engine::local::native::router(address, conn_tx, route_rx);
						conn_rx.into_recv_async().await??
					}

					#[cfg(not(feature = "kv-rocksdb"))]
					return Err(DbError::Ds(
						"Cannot connect to the `rocksdb` storage engine as it is not enabled in this build of SurrealDB".to_owned(),
					)
					.into());
				}

				"speedb" => {
					#[cfg(feature = "kv-speedb")]
					{
						features.insert(ExtraFeatures::Backup);
						engine::local::native::router(address, conn_tx, route_rx);
						conn_rx.into_recv_async().await??
					}

					#[cfg(not(feature = "kv-speedb"))]
					return Err(DbError::Ds(
						"Cannot connect to the `speedb` storage engine as it is not enabled in this build of SurrealDB".to_owned(),
					)
					.into());
				}

				"tikv" => {
					#[cfg(feature = "kv-tikv")]
					{
						features.insert(ExtraFeatures::Backup);
						engine::local::native::router(address, conn_tx, route_rx);
						conn_rx.into_recv_async().await??
					}

					#[cfg(not(feature = "kv-tikv"))]
					return Err(
						DbError::Ds("Cannot connect to the `tikv` storage engine as it is not enabled in this build of SurrealDB".to_owned()).into()
					);
				}

				"http" | "https" => {
					#[cfg(feature = "protocol-http")]
					{
						features.insert(ExtraFeatures::Auth);
						features.insert(ExtraFeatures::Backup);
						let headers = http::default_headers();
						#[allow(unused_mut)]
						let mut builder = ClientBuilder::new().default_headers(headers);
						#[cfg(any(feature = "native-tls", feature = "rustls"))]
						if let Some(tls) = address.tls_config {
							builder = match tls {
								#[cfg(feature = "native-tls")]
								Tls::Native(config) => builder.use_preconfigured_tls(config),
								#[cfg(feature = "rustls")]
								Tls::Rust(config) => builder.use_preconfigured_tls(config),
							};
						}
						let client = builder.build()?;
						let base_url = address.endpoint;
						engine::remote::http::health(
							client.get(base_url.join(Method::Health.as_str())?),
						)
						.await?;
						engine::remote::http::native::router(base_url, client, route_rx);
					}

					#[cfg(not(feature = "protocol-http"))]
					return Err(DbError::Ds(
						"Cannot connect to the `HTTP` remote engine as it is not enabled in this build of SurrealDB".to_owned(),
					)
					.into());
				}

				"ws" | "wss" => {
					#[cfg(feature = "protocol-ws")]
					{
						features.insert(ExtraFeatures::Auth);
						let url = address.endpoint.join(engine::remote::ws::PATH)?;
						#[cfg(any(feature = "native-tls", feature = "rustls"))]
						let maybe_connector = address.tls_config.map(Connector::from);
						#[cfg(not(any(feature = "native-tls", feature = "rustls")))]
						let maybe_connector = None;
						let config = WebSocketConfig {
							max_send_queue: match capacity {
								0 => None,
								capacity => Some(capacity),
							},
							max_message_size: Some(engine::remote::ws::native::MAX_MESSAGE_SIZE),
							max_frame_size: Some(engine::remote::ws::native::MAX_FRAME_SIZE),
							accept_unmasked_frames: false,
						};
						let socket = engine::remote::ws::native::connect(
							&url,
							Some(config),
							maybe_connector.clone(),
						)
						.await?;
						engine::remote::ws::native::router(
							url,
							maybe_connector,
							capacity,
							config,
							socket,
							route_rx,
						);
					}

					#[cfg(not(feature = "protocol-ws"))]
					return Err(DbError::Ds(
						"Cannot connect to the `WebSocket` remote engine as it is not enabled in this build of SurrealDB".to_owned(),
					)
					.into());
				}

				scheme => {
					return Err(Error::Scheme(scheme.to_owned()).into());
				}
			}

			Ok(Surreal {
				router: OnceCell::with_value(Arc::new(Router {
					features,
					conn: PhantomData,
					sender: route_tx,
					last_id: AtomicI64::new(0),
				})),
			})
		})
	}

	fn send<'r>(
		&'r mut self,
		router: &'r Router<Self>,
		param: Param,
	) -> Pin<Box<dyn Future<Output = Result<Receiver<Result<DbResponse>>>> + Send + Sync + 'r>> {
		Box::pin(async move {
			let (sender, receiver) = flume::bounded(1);
			self.id = router.next_id();
			let route = Route {
				request: (self.id, self.method, param),
				response: sender,
			};
			router.sender.send_async(Some(route)).await?;
			Ok(receiver)
		})
	}
}
