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
    pub fn ensure_monitored(&self, address: ServerAddress, token: String) {
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
            Self::monitor_loop(address_clone, token, tx, monitors_ref).await;
        });

        monitors.insert(address, task);
    }

    async fn monitor_loop(
        address: ServerAddress,
        token: String,
        tx: mpsc::Sender<ServerAddress>,
        monitors: Arc<Mutex<HashMap<ServerAddress, JoinHandle<()>>>>,
    ) {
        let mut interval: tokio::time::Interval = tokio::time::interval(Duration::from_secs(1));

        tokio::time::sleep(Duration::from_secs(5)).await;

        loop {
            interval.tick().await;

            match Self::check_server_started(&address.hostname, address.port as u16, token.clone())
                .await
            {
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

    async fn check_server_started(
        hostname: &str,
        management_port: u16,
        token: String,
    ) -> Result<bool, ServerMonitorError> {
        let url = format!("ws://{}:{}", hostname, management_port)
            .parse()
            .unwrap();

        let client = match WebsocketClient::new(url, Some(token)).await {
            Ok(client) => client,
            Err(e) => {
                return Err(ServerMonitorError::Custom(format!(
                    "Failed to connect websocket: {:?}",
                    e
                )));
            }
        };

        let status: ServerManagementStateResponse = client
            .send_request::<(), ServerManagementStateResponse>("minecraft:server/status", None)
            .await
            .expect("Failed to send subscription request");

        debug!("Received status: {:?}", status);

        client.close().await;

        Ok(status.started)
    }
}

#[derive(serde::Deserialize, Debug)]
struct ServerManagementStateResponse {
    started: bool,
}
