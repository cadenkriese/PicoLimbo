use crate::server::batch::Batch;
use crate::server::client_state::ClientState;
use crate::server::packet_handler::{PacketHandler, PacketHandlerError};
use crate::server::packet_registry::PacketRegistry;
use crate::server_state::ServerState;
use minecraft_packets::login::cookie_response_packet::CookieResponsePacket;
use minecraft_protocol::prelude::Nbt;
use pico_rpc::server_monitor::ServerAddress;

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

        if self.identifier.namespace != "pico_limbo" {
            return Err(PacketHandlerError::invalid_state(
                "Received a cookie from an unknown namespace",
            ));
        }

        if self.identifier.thing == "destination" {
            let payload_bytes = self.payload.inner().as_slice();
            let payload_tag = Nbt::from_bytes(payload_bytes)
                .map_err(|_| PacketHandlerError::invalid_state("Failed to parse cookie payload"))?;
            let hostname = payload_tag
                .find_tag("hostname")
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

            let address = ServerAddress { hostname, port };
            server_state.ensure_monitored(address.clone());
            client_state.set_destination(address);

            Ok(batch)
        } else {
            Err(PacketHandlerError::invalid_state(
                "Received a cookie with an unknown identifier",
            ))
        }
    }
}
