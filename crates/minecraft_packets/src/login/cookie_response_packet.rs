use minecraft_protocol::prelude::*;

#[derive(PacketIn)]
pub struct CookieResponsePacket {
    pub identifier: Identifier,
    pub payload: LengthPaddedVec<u8>,
}
