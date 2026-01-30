use nimiq_jsonrpc_client::Client;
use nimiq_jsonrpc_client::websocket::WebsocketClient;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ServerAddress {
    pub hostname: String,
    pub port: i32,
}

#[derive(Error, Debug)]
pub enum ServerMonitorError {
    #[error("An error occurred while monitoring a server: {0}")]
    Custom(String),
    #[error("{0}")]
    InvalidState(String),
}

#[derive(Clone)]
pub struct ServerMonitor {
    active_monitors: Arc<Mutex<HashMap<ServerAddress, JoinHandle<()>>>>,
    ready_tx: mpsc::Sender<ServerAddress>,
}

impl ServerMonitor {
    pub fn new(ready_tx: mpsc::Sender<ServerAddress>) -> Self {
        Self {
            active_monitors: Arc::new(Mutex::new(HashMap::new())),
            ready_tx,
        }
    }
}

impl Default for ServerMonitor {
    fn default() -> Self {
        let (tx, _) = mpsc::channel(1);
        Self::new(tx)
    }
}

impl ServerMonitor {
    pub fn ensure_monitored(&self, address: ServerAddress) {
        let mut monitors = self.active_monitors.lock().unwrap();

        if monitors.contains_key(&address) {
            return;
        }

        info!(
            "Monitoring {}:{} for startup",
            address.hostname, address.port
        );

        let address_clone = address.clone();
        let tx = self.ready_tx.clone();
        let monitors_ref = self.active_monitors.clone();

        let task = tokio::spawn(async move {
            Self::monitor_loop(address_clone, tx, monitors_ref).await;
        });

        monitors.insert(address, task);
    }

    async fn monitor_loop(
        address: ServerAddress,
        tx: mpsc::Sender<ServerAddress>,
        monitors: Arc<Mutex<HashMap<ServerAddress, JoinHandle<()>>>>,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;

            match Self::server_started(&address.hostname, address.port as u16).await {
                Ok(started) => {
                    if started {
                        debug!("Server {}:{} has started", address.hostname, address.port);

                        if let Err(e) = tx.send(address.clone()).await {
                            error!("Failed to report ready server: {}", e);
                        }

                        break;
                    } else {
                        debug!(
                            "Server {}:{} not started yet",
                            address.hostname, address.port
                        );
                    }
                }
                Err(e) => {
                    debug!(
                        "Error checking server {}:{}: {:?}",
                        address.hostname, address.port, e
                    );
                }
            }
        }

        let mut lock = monitors.lock().unwrap();
        lock.remove(&address);
    }

    async fn server_started(hostname: &str, port: u16) -> Result<bool, ServerMonitorError> {
        let url = format!("ws://{}:{}", hostname, port).parse().unwrap();
        let token = Some("krQJUjIjSPwmcXKi3cIearBh65fH3TEEwzrQ6D2o".to_string());

        let client = match WebsocketClient::new(url, token).await {
            Ok(client) => client,
            Err(e) => {
                return Err(ServerMonitorError::Custom(format!(
                    "Failed to connect websocket: {:?}",
                    e
                )));
            }
        };

        let status: ServerManagementStateResponse = client
            .send_request("minecraft:notification/status", &())
            .await
            .expect("Failed to send subscription request");

        debug!("Received status: {:?}", status);

        return Ok(status.started);
    }
}

#[derive(serde::Deserialize, Debug)]
struct ServerManagementVersion {
    protocol: i32,
    name: String,
}

#[derive(serde::Deserialize, Debug)]
struct ServerManagementPlayer {
    name: String,
    id: String,
}

#[derive(serde::Deserialize, Debug)]
struct ServerManagementStateResponse {
    players: Vec<ServerManagementPlayer>,
    started: bool,
    version: ServerManagementVersion,
}
