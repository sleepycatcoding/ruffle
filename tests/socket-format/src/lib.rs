use std::{fs::File, io, path::Path};

use serde::{Deserialize, Serialize};
use serde_json::from_reader;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Endian {
    Little,
    Big,
}

// See https://github.com/serde-rs/serde/issues/368
const fn bool_false() -> bool {
    false
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RawValue {
    Float {
        val: f32,
        endian: Endian,
    },

    Double {
        val: f64,
        endian: Endian,
    },

    Int {
        val: i32,
        endian: Endian,
    },

    UnsignedInt {
        val: u32,
        endian: Endian,
    },

    Short {
        val: i16,
        endian: Endian,
    },

    UnsignedShort {
        val: u16,
        endian: Endian,
    },

    Byte {
        val: i8,
    },

    UnsignedByte {
        val: u8,
    },

    Boolean {
        val: bool,
    },

    Bytes {
        val: Vec<u8>,
    },

    String {
        val: String,
        #[serde(default = "bool_false")]
        null_terminated: bool,
    },
}

impl RawValue {
    pub fn to_bytes(self) -> Vec<u8> {
        macro_rules! match_arm_impl {
            ($val:expr, $endian:expr) => {
                match $endian {
                    Endian::Big => $val.to_be_bytes().to_vec(),
                    Endian::Little => $val.to_le_bytes().to_vec(),
                }
            };
        }

        match self {
            RawValue::Float { val, endian } => match_arm_impl!(val, endian),
            RawValue::Double { val, endian } => match_arm_impl!(val, endian),
            RawValue::Int { val, endian } => match_arm_impl!(val, endian),
            RawValue::UnsignedInt { val, endian } => match_arm_impl!(val, endian),
            RawValue::Short { val, endian } => match_arm_impl!(val, endian),
            RawValue::UnsignedShort { val, endian } => match_arm_impl!(val, endian),
            // NOTE - For these next two, the endianness does not matter, so we just use Little Endian
            RawValue::Byte { val } => match_arm_impl!(val, Endian::Little),
            RawValue::UnsignedByte { val } => match_arm_impl!(val, Endian::Little),
            RawValue::Boolean { val } => vec![val as u8],
            RawValue::Bytes { val } => val,
            RawValue::String {
                val,
                null_terminated,
            } => {
                let mut output = vec![];

                output.extend_from_slice(val.as_bytes());

                if null_terminated {
                    output.push(0);
                }

                output
            }
        }
    }
}

pub trait VecExt {
    fn to_bytes(self) -> Vec<u8>;
}

impl VecExt for Vec<RawValue> {
    fn to_bytes(self) -> Vec<u8> {
        let mut output = vec![];

        for val in self {
            output.extend(val.to_bytes());
        }

        output
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SocketEvent {
    /// Wait for input data that matches this.
    Receive { expected: Vec<u8> },
    /// Send data to client.
    Send { payload: Vec<RawValue> },
    /// Expect client to disconnect.
    WaitForDisconnect,
    /// Disconnect the client.
    Disconnect,
}

impl SocketEvent {
    pub fn from_file<P>(path: P) -> Result<Vec<Self>, io::Error>
    where
        P: AsRef<Path>,
    {
        let file = File::open(path)?;

        Ok(from_reader(file)?)
    }
}
