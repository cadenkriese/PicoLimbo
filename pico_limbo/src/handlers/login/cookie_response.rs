use crate::server::batch::Batch;
use crate::server::client_state::ClientState;
use crate::server::packet_handler::{PacketHandler, PacketHandlerError};
use crate::server::packet_registry::PacketRegistry;
use crate::server_state::ServerState;
use minecraft_packets::login::cookie_response_packet::CookieResponsePacket;
use minecraft_protocol::prelude::{Nbt, Optional};
use pico_rpc::server_monitor::ServerAddress;
use tracing::{debug, error};

impl PacketHandler for CookieResponsePacket {
    fn handle(
        &self,
        client_state: &mut ClientState,
        server_state: &ServerState,
    ) -> Result<Batch<PacketRegistry>, PacketHandlerError> {
        let batch = Batch::new();

        if !server_state.accept_transfers() {
            return Err(PacketHandlerError::invalid_state("Transfers disabled"));
        }

        let payload_bytes = match &self.payload {
            Optional::None => {
                return Err(PacketHandlerError::invalid_state(
                    "Cookie payload is missing",
                ));
            }
            Optional::Some(bytes) => bytes.inner().as_slice(),
        };

        let payload_hex = payload_bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>();
        debug!("Received cookie {} with payload {}", self.identifier, payload_hex);

        if self.identifier.namespace != "pico_limbo" {
            return Err(PacketHandlerError::invalid_state(
                "Received a cookie from an unknown namespace",
            ));
        }

        if self.identifier.thing == "destination" {
            let payload_tag = Nbt::from_network_bytes(payload_bytes).map_err(|e| {
                error!("Failed to decode NBT payload: {}", e);
                PacketHandlerError::invalid_state("Invalid NBT in cookie payload")
            })?;
            
            let hostname = payload_tag
                .find_tag("host")
                .and_then(|tag| tag.get_string())
                .ok_or_else(|| {
                    PacketHandlerError::invalid_state("Cookie payload missing 'hostname' tag")
                })?;
            let port = payload_tag
                .find_tag("port")
                .and_then(|tag| tag.get_int())
                .ok_or_else(|| {
                    PacketHandlerError::invalid_state("Cookie payload missing 'port' tag")
                })?;

            let management_address = ServerAddress { hostname: hostname.clone(), port: server_state.external_server_management_port() };
            let game_server_address = ServerAddress { hostname, port };
            server_state.ensure_monitored(management_address.clone(), server_state.external_server_management_secret().clone());
            client_state.set_destination(game_server_address);

            Ok(batch)
        } else {
            Err(PacketHandlerError::invalid_state(
                "Received a cookie with an unknown identifier",
            ))
        }
    }
}
