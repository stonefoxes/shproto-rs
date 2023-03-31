#![no_std]

use heapless;

// 0xFF 0xFE 0x03 0x00 0x01 0x40 0xa5 <- HEX
// 255  254  3    0    1    64   165  <- u8
// \377 \376 \003 \000 \001 @    \245 <- ASCII

#[derive(Debug, Copy, Clone)]
pub enum ShprotoError {
    PushFailed
}

pub enum ControlByte {}
impl ControlByte {
    pub const START: u8 = 0xFE;
    pub const ESCAPE: u8 = 0xFD;
    pub const STOP: u8 = 0xA5;
}

pub struct ShprotoPacket<const N: usize = 256> {
    data: heapless::Vec<u8, N>,
    crc: u16,
    completed: bool,
    valid: bool,
}
impl<const N: usize> ShprotoPacket<N> {
    pub fn new() -> Self {
        let mut p = ShprotoPacket {
            data: Default::default(),
            crc: 0xFFFF,
            completed: false,
            valid: false
        };
        p.data.push(0xFF).unwrap();
        p.data.push(0xFE).unwrap();
        p
    }
    fn crc16(&mut self, byte: u8) {
        let mut crc = self.crc ^ (byte as u16);
        for _ in 0..8 {
            if (crc & 0x0001) != 0 {
                crc = (crc >> 1) ^ 0xA001
            } else {
                crc = crc >> 1
            }
        }
        self.crc = crc;
    }

    pub fn start(&mut self, command: u8) -> Result<(), ShprotoError> {
        self.add_byte(command)
    }

    pub fn add_byte(&mut self, byte: u8) -> Result<(), ShprotoError>{
        // calculate crc
        self.crc16(byte);
        // push byte
        let need_escape: bool = match byte {
            ControlByte::START | ControlByte::ESCAPE | ControlByte::STOP => true,
            _ => false
        };
        if need_escape {
            self.data.push(ControlByte::ESCAPE)
                .map_err(|e| ShprotoError::PushFailed)?;
            self.data.push((!byte) & 0xFF)
                .map_err(|e| ShprotoError::PushFailed)?;
        } else {
            self.data.push(byte)
                .map_err(|e| ShprotoError::PushFailed)?;;
        }
        Ok(())
    }

    pub fn complete(&mut self) {
        // get CRC bytes
        for byte in self.crc.to_le_bytes().iter() {
            self.add_byte(*byte).unwrap()
        }
        self.data.push(ControlByte::STOP).unwrap();
        self.completed = true;
        if self.crc == 0 {
            self.valid = true;
        }
    }
}

// Packet
// PacketBuilder
// Encoder
// Decoder
// StreamDecoder

enum ShprotoParserState {
    Start,
    Data,
    EscapedData,
    CrcLow,
    CrcHigh,
    Stop,
}

struct ShprotoParser<const N: usize> {
    state: ShprotoParserState,
    packet: ShprotoPacket<N>,
}

impl<const N: usize> ShprotoParser<N> {
    fn new() -> Self {
        ShprotoParser {
            state: ShprotoParserState::Start,
            packet: ShprotoPacket::new(),
        }
    }

    fn parse_byte(&mut self, byte: u8) -> Result<Option<ShprotoPacket<N>>, ShprotoError> {
        match self.state {
            ShprotoParserState::Start => {
                if byte == ControlByte::START {
                    self.packet = ShprotoPacket::new();
                    self.state = ShprotoParserState::Data;
                }
            }
            ShprotoParserState::Data => {
                match byte {
                    ControlByte::START => {
                        self.packet = ShprotoPacket::new();
                        self.state = ShprotoParserState::Data;
                    }
                    ControlByte::ESCAPE => {
                        self.state = ShprotoParserState::EscapedData;
                    }
                    ControlByte::STOP => {
                        self.state = ShprotoParserState::CrcLow;
                    }
                    _ => {
                        self.packet.add_byte(byte)?;
                    }
                }
            }
            ShprotoParserState::EscapedData => {
                let unescaped_byte = (!byte) & 0xFF;
                self.packet.add_byte(unescaped_byte)?;
                self.state = ShprotoParserState::Data;
            }
            ShprotoParserState::CrcLow => {
                self.packet.add_byte(byte)?;
                self.state = ShprotoParserState::CrcHigh;
            }
            ShprotoParserState::CrcHigh => {
                // Create a new packet to return.
                let completed_packet = ShprotoPacket {
                    data: self.packet.data.clone(),
                    crc: self.packet.crc,
                    completed: true,
                    valid: self.packet.crc == 0,
                };

                // Reset the parser state and return the completed packet.
                self.state = ShprotoParserState::Start;
                self.packet = ShprotoPacket::new();
                return Ok(Some(completed_packet));
            }
            ShprotoParserState::Stop => {}
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc16_works() {
        let mut packet = ShprotoPacket::<256>::new();
        packet.start(0x03).unwrap();
        packet.add_byte(0x99).unwrap();
        assert_eq!(packet.crc, 10945);
        packet.complete();
        assert_eq!(packet.crc, 0);
        assert_eq!(packet.completed, true);
        assert_eq!(packet.valid, true);
    }
    #[test]
    fn parse() {
        let bytes:[u8; 7] = [0xFF, 0xFE, 0x03, 0x00, 0x01, 0x40, 0xa5];
        let mut parser = ShprotoParser::<256>::new();
        for byte in bytes.as_slice() {
            if let Some(packet) = parser.parse_byte(*byte).unwrap() {
                assert_eq!(packet.crc, 0);
                assert_eq!(packet.data, bytes);
            }
        }
    }
}