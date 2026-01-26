use minecraft_protocol::prelude::*;

#[derive(PacketOut)]
pub struct CookieRequestPacket {
    pub identifier: Identifier,
}
