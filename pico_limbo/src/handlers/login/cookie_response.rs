use std::fmt::Write;

use crate::server::batch::Batch;
use crate::server::client_state::ClientState;
use crate::server::packet_handler::{PacketHandler, PacketHandlerError};
use crate::server::packet_registry::PacketRegistry;
use crate::server_state::ServerState;
use minecraft_packets::login::cookie_response_packet::CookieResponsePacket;
use minecraft_protocol::prelude::Optional;
use pico_nbt::{NbtOptions, Value};
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

        let payload_hex = payload_bytes.iter().fold(String::new(), |mut acc, b| {
            write!(&mut acc, "{b:02x}").unwrap();
            acc
        });
        debug!(
            "Received cookie {} with payload {}",
            self.identifier, payload_hex
        );

        if self.identifier.namespace != "pico_limbo" {
            return Err(PacketHandlerError::invalid_state(
                "Received a cookie from an unknown namespace",
            ));
        }

        if self.identifier.thing == "destination" {
            let (_, payload_tag) = pico_nbt::from_slice_with_options(
                payload_bytes,
                NbtOptions::new().nameless_root(true),
            )
            .map_err(|e| {
                error!("Failed to decode NBT payload: {}", e);
                PacketHandlerError::invalid_state("Invalid NBT in cookie payload")
            })?;

            let compound = match &payload_tag {
                Value::Compound(map) => map,
                _ => return Err(PacketHandlerError::invalid_state("Cookie payload is not a compound tag")),
            };

            let hostname = compound
                .get("host")
                .and_then(|v| if let Value::String(s) = v { Some(s.clone()) } else { None })
                .ok_or_else(|| {
                    PacketHandlerError::invalid_state("Cookie payload missing 'host' tag")
                })?;
            let port = compound
                .get("port")
                .and_then(|v| v.get_int())
                .ok_or_else(|| {
                    PacketHandlerError::invalid_state("Cookie payload missing 'port' tag")
                })?;

            let management_address = ServerAddress {
                hostname: server_state
                    .external_server_hostname()
                    .unwrap_or(hostname.clone()),
                port: server_state.external_server_management_port(),
            };
            let game_server_address = ServerAddress { hostname, port };
            server_state.ensure_monitored(
                game_server_address.clone(),
                management_address,
                server_state.external_server_management_secret(),
            );
            client_state.set_destination(game_server_address);

            Ok(batch)
        } else {
            Err(PacketHandlerError::invalid_state(
                "Received a cookie with an unknown identifier",
            ))
        }
    }
}
