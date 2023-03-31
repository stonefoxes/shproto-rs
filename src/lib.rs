#![no_std]

use heapless;

pub fn crc16(crc: u16, byte: u8) -> u16 {
    let mut crc = crc ^ (byte as u16);
    for _ in 0..8 {
        if (crc & 0x0001) != 0 {
            crc = (crc >> 1) ^ 0xA001
        } else {
            crc = crc >> 1
        }
    }
    crc
}

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

#[derive(Debug)]
pub struct ShprotoPacket<const N: usize = 256> {
    pub data: heapless::Vec<u8, N>,
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

    pub fn start(&mut self, command: u8) -> Result<(), ShprotoError> {
        self.add_byte(command)
    }

    pub fn add_byte(&mut self, byte: u8) -> Result<(), ShprotoError>{
        // calculate crc
        self.crc = crc16(self.crc, byte);
        // push byte
        let need_escape: bool = match byte {
            ControlByte::START | ControlByte::ESCAPE | ControlByte::STOP => true,
            _ => false
        };
        if need_escape {
            self.data.push(ControlByte::ESCAPE)
                .map_err(|_| ShprotoError::PushFailed)?;
            self.data.push((!byte) & 0xFF)
                .map_err(|_| ShprotoError::PushFailed)?;
        } else {
            self.data.push(byte)
                .map_err(|_| ShprotoError::PushFailed)?;
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

enum ShprotoParserState {
    Start,
    Data,
    EscapedData,
}

pub struct ShprotoParser<const N: usize> {
    state: ShprotoParserState,
    packet: ShprotoPacket<N>,
}

impl<const N: usize> ShprotoParser<N> {
    pub fn new() -> Self {
        ShprotoParser {
            state: ShprotoParserState::Start,
            packet: ShprotoPacket::new(),
        }
    }

    pub fn parse_byte(&mut self, byte: u8) -> Result<Option<ShprotoPacket<N>>, ShprotoError> {
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
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build() {
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
        let bytes = [0xFF, 0xFE, 0x69, 0x8C, 0x90, 0x8C, 0x89, 0xFD, 0x5A, 0x53, 0xFD, 0x02, 0xA5];
        let mut parser = ShprotoParser::<4096>::new();
        let mut packet_counter: u32 = 0;
        for byte in bytes.as_slice() {
            if let Some(packet) = parser.parse_byte(*byte).unwrap() {
                assert_eq!(packet.crc, 0);
                packet_counter += 1;
            }
        }
        assert_eq!(packet_counter, 1);
    }
}