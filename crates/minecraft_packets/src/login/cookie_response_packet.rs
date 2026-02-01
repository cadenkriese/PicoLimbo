use minecraft_protocol::prelude::*;

#[derive(PacketIn)]
pub struct CookieResponsePacket {
    pub identifier: Identifier,
    pub payload: Optional<LengthPaddedVec<u8>>,
}
