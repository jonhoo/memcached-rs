// Copyright (c) 2015 Y. T. Chung <zonyitoo@gmail.com>
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>,
// at your option. All files in the project carrying such
// notice may not be copied, modified, or distributed except
// according to those terms.

use std::io::{BufRead, Cursor, BufReader, Write};
use std::string::String;
use std::str;
use std::collections::{BTreeMap, HashMap};
use std::convert::From;
use std::error;
use std::fmt;

use byteorder::{WriteBytesExt, ReadBytesExt, BigEndian};

use proto::{Operation, MultiOperation, ServerOperation, NoReplyOperation, CasOperation, AuthOperation};
use proto::{self, MemCachedResult, AuthResponse};
use proto::binarydef::{RequestHeader, RequestPacket, RequestPacketRef, ResponsePacket, Command, DataType};

use semver::Version;

use rand::random;

pub use proto::binarydef::Status;

#[derive(Debug, Clone)]
pub struct Error {
    status: Status,
    desc: &'static str,
    detail: Option<String>,
}

impl Error {
    fn from_status(status: Status, detail: Option<String>) -> Error {
        Error {
            status: status,
            desc: status.desc(),
            detail: detail,
        }
    }

    pub fn detail(&self) -> Option<String> {
        self.detail.clone()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "{}", self.desc));
        match self.detail {
            Some(ref s) => write!(f, " ({})", s),
            None => Ok(())
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        self.desc
    }
}

pub struct BinaryProto<T: BufRead + Write + Send> {
    stream: T,
}

// impl<T: BufRead + Write + Send> Proto for BinaryProto<T> {
//     fn clone(&self) -> Box<Proto + Send> {
//         box BinaryProto { stream: BufStream::new(self.stream.get_ref().clone()) }
//     }
// }

impl<T: BufRead + Write + Send> BinaryProto<T> {
    pub fn new(stream: T) -> BinaryProto<T> {
        BinaryProto {
            stream: stream,
        }
    }

    fn send_noop(&mut self) -> MemCachedResult<u32> {
        let opaque = random::<u32>();
        debug!("Sending NOOP");
        let req_packet = RequestPacket::new(
                                Command::Noop, DataType::RawBytes, 0, opaque, 0,
                                Vec::new(),
                                Vec::new(),
                                Vec::new());

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        Ok(opaque)
    }
}

impl<T: BufRead + Write + Send> Operation for BinaryProto<T> {
    fn set(&mut self, key: &[u8], value: &[u8], flags: u32, expiration: u32) -> MemCachedResult<()> {
        let opaque = random::<u32>();
        debug!("Set key: {:?} {:?}, value: {:?}, flags: 0x{:x}, expiration: {}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), value, flags, expiration);
        let mut extra = [0u8; 8];
        {
            let mut extra_buf = Cursor::new(&mut extra[..]);
            try!(extra_buf.write_u32::<BigEndian>(flags));
            try!(extra_buf.write_u32::<BigEndian>(expiration));
        }

        let req_header = RequestHeader::from_payload(Command::Set, DataType::RawBytes, 0, opaque, 0,
                                                     key, &extra, value);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &extra,
                                key,
                                value);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => Ok(()),
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn add(&mut self, key: &[u8], value: &[u8], flags: u32, expiration: u32) -> MemCachedResult<()> {
        let opaque = random::<u32>();
        debug!("Add key: {:?} {:?}, value: {:?}, flags: 0x{:x}, expiration: {}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), value, flags, expiration);
        let mut extra = [0u8; 8];
        {
            let mut extra_buf = Cursor::new(&mut extra[..]);
            try!(extra_buf.write_u32::<BigEndian>(flags));
            try!(extra_buf.write_u32::<BigEndian>(expiration));
        }

        let req_header = RequestHeader::from_payload(Command::Add, DataType::RawBytes, 0, opaque, 0,
                                                     key, &extra, value);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &extra,
                                key,
                                value);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => Ok(()),
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn delete(&mut self, key: &[u8]) -> MemCachedResult<()> {
        let opaque = random::<u32>();
        debug!("Delete key: {:?} {:?}", key, str::from_utf8(key).unwrap_or("<not-utf8-key>"));
        let req_header = RequestHeader::from_payload(Command::Delete, DataType::RawBytes, 0, opaque, 0,
                                                     key, &[], &[]);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &[],
                                key,
                                &[]);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => Ok(()),
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn replace(&mut self, key: &[u8], value: &[u8], flags: u32, expiration: u32) -> MemCachedResult<()> {
        let opaque = random::<u32>();
        debug!("Replace key: {:?} {:?}, value: {:?}, flags: 0x{:x}, expiration: {}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), value, flags, expiration);
        let mut extra = [0u8; 8];
        {
            let mut extra_buf = Cursor::new(&mut extra[..]);
            try!(extra_buf.write_u32::<BigEndian>(flags));
            try!(extra_buf.write_u32::<BigEndian>(expiration));
        }

        let req_header = RequestHeader::from_payload(Command::Replace, DataType::RawBytes, 0, opaque, 0,
                                                     key, &extra, value);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &extra,
                                key,
                                value);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => Ok(()),
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn get(&mut self, key: &[u8]) -> MemCachedResult<(Vec<u8>, u32)> {
        let opaque = random::<u32>();
        debug!("Get key: {:?} {:?}", key, str::from_utf8(key).unwrap_or("<not-utf8-key>"));
        let req_header = RequestHeader::from_payload(Command::Get, DataType::RawBytes, 0, opaque, 0,
                                                     key, &[], &[]);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &[],
                                key,
                                &[]);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => {
                let mut extrabufr = BufReader::new(&resp.extra[..]);
                let flags = try!(extrabufr.read_u32::<BigEndian>());

                Ok((resp.value, flags))
            },
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn getk(&mut self, key: &[u8]) -> MemCachedResult<(Vec<u8>, Vec<u8>, u32)> {
        let opaque = random::<u32>();
        debug!("GetK key: {:?} {:?}", key, str::from_utf8(key).unwrap_or("<not-utf8-key>"));
        let req_header = RequestHeader::from_payload(Command::GetKey, DataType::RawBytes, 0, opaque, 0,
                                                     key, &[], &[]);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &[],
                                key,
                                &[]);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => {
                let mut extrabufr = BufReader::new(&resp.extra[..]);
                let flags = try!(extrabufr.read_u32::<BigEndian>());

                Ok((resp.key, resp.value, flags))
            },
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn increment(&mut self, key: &[u8], amount: u64, initial: u64, expiration: u32) -> MemCachedResult<u64> {
        let opaque = random::<u32>();
        debug!("Increment key: {:?} {:?}, amount: {}, initial: {}, expiration: {}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), amount, initial, expiration);
        let mut extra = [0u8; 20];
        {
            let mut extra_buf = Cursor::new(&mut extra[..]);
            try!(extra_buf.write_u64::<BigEndian>(amount));
            try!(extra_buf.write_u64::<BigEndian>(initial));
            try!(extra_buf.write_u32::<BigEndian>(expiration));
        }

        let req_header = RequestHeader::from_payload(Command::Increment, DataType::RawBytes, 0, opaque, 0,
                                                     key, &extra, &[]);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &extra,
                                key,
                                &[]);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => {
                let mut bufr = BufReader::new(&resp.value[..]);
                Ok(try!(bufr.read_u64::<BigEndian>()))
            },
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn decrement(&mut self, key: &[u8], amount: u64, initial: u64, expiration: u32) -> MemCachedResult<u64> {
        let opaque = random::<u32>();
        debug!("Decrement key: {:?} {:?}, amount: {}, initial: {}, expiration: {}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), amount, initial, expiration);
        let mut extra = [0u8; 20];
        {
            let mut extra_buf = Cursor::new(&mut extra[..]);
            try!(extra_buf.write_u64::<BigEndian>(amount));
            try!(extra_buf.write_u64::<BigEndian>(initial));
            try!(extra_buf.write_u32::<BigEndian>(expiration));
        }

        let req_header = RequestHeader::from_payload(Command::Decrement, DataType::RawBytes, 0, opaque, 0,
                                                     key, &extra, &[]);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &extra,
                                key,
                                &[]);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => {
                let mut bufr = BufReader::new(&resp.value[..]);
                Ok(try!(bufr.read_u64::<BigEndian>()))
            },
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn append(&mut self, key: &[u8], value: &[u8]) -> MemCachedResult<()> {
        let opaque = random::<u32>();
        debug!("Append key: {:?} {:?}, value: {:?}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), value);
        let req_header = RequestHeader::from_payload(Command::Append, DataType::RawBytes, 0, opaque, 0,
                                                     key, &[], value);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &[],
                                key,
                                value);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => Ok(()),
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn prepend(&mut self, key: &[u8], value: &[u8]) -> MemCachedResult<()> {
        let opaque = random::<u32>();
        debug!("Prepend key: {:?} {:?}, value: {:?}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), value);
        let req_header = RequestHeader::from_payload(Command::Prepend, DataType::RawBytes, 0, opaque, 0,
                                                     key, &[], value);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &[],
                                key,
                                value);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => Ok(()),
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn touch(&mut self, key: &[u8], expiration: u32) -> MemCachedResult<()> {
        let opaque = random::<u32>();
        debug!("Touch key: {:?} {:?}, expiration: {}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), expiration);
        let mut extra = [0u8; 4];
        {
            let mut extra_buf = Cursor::new(&mut extra[..]);
            try!(extra_buf.write_u32::<BigEndian>(expiration));
        }

        let req_header = RequestHeader::from_payload(Command::Touch, DataType::RawBytes, 0, opaque, 0,
                                                     key, &extra, &[]);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &extra,
                                key,
                                &[]);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => Ok(()),
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }
}

impl<T: BufRead + Write + Send> ServerOperation for BinaryProto<T> {
    fn quit(&mut self) -> MemCachedResult<()> {
        let opaque = random::<u32>();
        debug!("Quit");
        let req_header = RequestHeader::from_payload(Command::Quit, DataType::RawBytes, 0, opaque, 0,
                                                     &[], &[], &[]);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &[],
                                &[],
                                &[]);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => Ok(()),
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn flush(&mut self, expiration: u32) -> MemCachedResult<()> {
        let opaque = random::<u32>();
        debug!("Expiration flush: {}", expiration);
        let mut extra = [0u8; 4];
        {
            let mut extra_buf = Cursor::new(&mut extra[..]);
            try!(extra_buf.write_u32::<BigEndian>(expiration));
        }

        let req_header = RequestHeader::from_payload(Command::Flush, DataType::RawBytes, 0, opaque, 0,
                                                     &[], &extra, &[]);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &extra,
                                &[],
                                &[]);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => Ok(()),
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn noop(&mut self) -> MemCachedResult<()> {
        debug!("Noop");
        let opaque = try!(self.send_noop());
        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => Ok(()),
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn version(&mut self) -> MemCachedResult<Version> {
        let opaque = random::<u32>();
        debug!("Version");
        let req_header = RequestHeader::new(Command::Version, DataType::RawBytes, 0, opaque, 0, 0, 0, 0);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &[],
                                &[],
                                &[]);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => {
                let val = resp.value;
                let verstr = match str::from_utf8(&val[..]) {
                    Ok(vs) => vs,
                    Err(..) => return Err(proto::Error::OtherError {
                        desc: "Response is not a string",
                        detail: None,
                    }),
                };

                Ok(match Version::parse(verstr) {
                    Ok(v) => v,
                    Err(err) => return Err(proto::Error::OtherError {
                        desc: "Unrecognized version string",
                        detail: Some(err.to_string()),
                    }),
                })
            },
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn stat(&mut self) -> MemCachedResult<BTreeMap<String, String>> {
        let opaque = random::<u32>();
        debug!("Stat");
        let req_header = RequestHeader::new(Command::Stat, DataType::RawBytes, 0, opaque, 0, 0, 0, 0);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &[],
                                &[],
                                &[]);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut result = BTreeMap::new();
        loop {
            let resp = try!(ResponsePacket::read_from(&mut self.stream));
            if resp.header.opaque != opaque {
                debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
                continue;
            }
            match resp.header.status {
                Status::NoError => {},
                _ => return Err(From::from(Error::from_status(resp.header.status, None))),
            }

            if resp.key.len() == 0 && resp.value.len() == 0 {
                break;
            }

            let key = match String::from_utf8(resp.key) {
                Ok(k) => k,
                Err(..) => return Err(proto::Error::OtherError {
                        desc: "Key is not a string",
                        detail: None,
                    }),
            };

            let val = match String::from_utf8(resp.value) {
                Ok(k) => k,
                Err(..) => return Err(proto::Error::OtherError {
                        desc: "Value is not a string",
                        detail: None,
                    }),
            };

            result.insert(key, val);
        }

        Ok(result)
    }
}

impl<T: BufRead + Write + Send> MultiOperation for BinaryProto<T> {
    fn set_multi(&mut self, kv: BTreeMap<&[u8], (&[u8], u32, u32)>) -> MemCachedResult<()> {
        for (key, (value, flags, expiration)) in kv.into_iter() {
            let mut extra = [0u8; 8];
            {
                let mut extra_buf = Cursor::new(&mut extra[..]);
                try!(extra_buf.write_u32::<BigEndian>(flags));
                try!(extra_buf.write_u32::<BigEndian>(expiration));
            }

            let req_header = RequestHeader::from_payload(Command::SetQuietly, DataType::RawBytes, 0, 0, 0,
                                                         key, &extra, value);
            let req_packet = RequestPacketRef::new(
                                    &req_header,
                                    &extra,
                                    key,
                                    value);

            try!(req_packet.write_to(&mut self.stream));
        }
        try!(self.send_noop());

        loop {
            let resp = try!(ResponsePacket::read_from(&mut self.stream));

            match resp.header.status {
                Status::NoError => {},
                _ => return Err(From::from(Error::from_status(resp.header.status, None))),
            }

            if resp.header.command == Command::Noop {
                return Ok(())
            }
        }
    }

    fn delete_multi(&mut self, keys: &[&[u8]]) -> MemCachedResult<()> {
        for key in keys.iter() {
            let req_header = RequestHeader::from_payload(Command::DeleteQuietly, DataType::RawBytes, 0, 0, 0,
                                                         *key, &[], &[]);
            let req_packet = RequestPacketRef::new(
                                    &req_header,
                                    &[],
                                    *key,
                                    &[]);

            try!(req_packet.write_to(&mut self.stream));
        }
        try!(self.send_noop());

        loop {
            let resp = try!(ResponsePacket::read_from(&mut self.stream));

            match resp.header.status {
                Status::NoError | Status::KeyNotFound => {},
                _ => return Err(From::from(Error::from_status(resp.header.status, None))),
            }

            if resp.header.command == Command::Noop {
                return Ok(());
            }
        }
    }

    fn increment_multi<'a>(&mut self, kv: HashMap<&'a [u8], (u64, u64, u32)>) -> MemCachedResult<HashMap<&'a [u8], u64>> {
        let opaques: MemCachedResult<HashMap<_, _>> = kv.into_iter().map(|(key, (amount, initial, expiration))| {
            let opaque = random::<u32>();
            let mut extra = [0u8; 20];
            {
                let mut extra_buf = Cursor::new(&mut extra[..]);
                try!(extra_buf.write_u64::<BigEndian>(amount));
                try!(extra_buf.write_u64::<BigEndian>(initial));
                try!(extra_buf.write_u32::<BigEndian>(expiration));
            }

            let req_header = RequestHeader::from_payload(Command::Increment, DataType::RawBytes, 0, opaque, 0,
                                                         key, &extra, &[]);
            let req_packet = RequestPacketRef::new(
                                    &req_header,
                                    &extra,
                                    key,
                                    &[]);

            try!(req_packet.write_to(&mut self.stream));
            Ok((opaque, key))
        }).collect();

        let opaques = try!(opaques);

        try!(self.send_noop());
        try!(self.stream.flush());

        let mut results = HashMap::with_capacity(opaques.len());
        loop {
            let resp = try!(ResponsePacket::read_from(&mut self.stream));
            match resp.header.status {
                Status::NoError => {},
                _ => return Err(From::from(Error::from_status(resp.header.status, None))),
            }

            if resp.header.command == Command::Noop {
                return Ok(results);
            }

            match opaques.get(&resp.header.opaque) {
                Some(&key) => {
                    let mut bufr = BufReader::new(&resp.value[..]);
                    let val = try!(bufr.read_u64::<BigEndian>());
                    results.insert(key, val);
                }
                None => {}
            }
        }
    }

    fn get_multi(&mut self, keys: &[&[u8]]) -> MemCachedResult<HashMap<Vec<u8>, (Vec<u8>, u32)>> {

        for key in keys.iter() {
            let req_header = RequestHeader::from_payload(Command::GetKeyQuietly, DataType::RawBytes, 0, 0, 0,
                                                         *key, &[], &[]);
            let req_packet = RequestPacketRef::new(
                                    &req_header,
                                    &[],
                                    *key,
                                    &[]);

            try!(req_packet.write_to(&mut self.stream));
        }
        try!(self.send_noop());

        let mut result = HashMap::with_capacity(keys.len());
        loop {
            let resp = try!(ResponsePacket::read_from(&mut self.stream));
            match resp.header.status {
                Status::NoError => {},
                _ => return Err(From::from(Error::from_status(resp.header.status, None))),
            }

            if resp.header.command == Command::Noop {
                return Ok(result);
            }

            let mut extrabufr = BufReader::new(&resp.extra[..]);
            let flags = try!(extrabufr.read_u32::<BigEndian>());

            result.insert(resp.key, (resp.value, flags));
        }
    }
}

impl<T: BufRead + Write + Send> NoReplyOperation for BinaryProto<T> {
    fn set_noreply(&mut self, key: &[u8], value: &[u8], flags: u32, expiration: u32) -> MemCachedResult<()> {
        let opaque = random::<u32>();
        debug!("Set noreply key: {:?} {:?}, value: {:?}, flags: 0x{:x}, expiration: {}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), value, flags, expiration);
        let mut extra = [0u8; 8];
        {
            let mut extra_buf = Cursor::new(&mut extra[..]);
            try!(extra_buf.write_u32::<BigEndian>(flags));
            try!(extra_buf.write_u32::<BigEndian>(expiration));
        }

        let req_header = RequestHeader::from_payload(Command::SetQuietly, DataType::RawBytes, 0, opaque, 0,
                                                     key, &extra, value);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &extra,
                                key,
                                value);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        Ok(())
    }

    fn add_noreply(&mut self, key: &[u8], value: &[u8], flags: u32, expiration: u32) -> MemCachedResult<()> {
        let opaque = random::<u32>();
        debug!("Add noreply key: {:?} {:?}, value: {:?}, flags: 0x{:x}, expiration: {}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), value, flags, expiration);
        let mut extra = [0u8; 8];
        {
            let mut extra_buf = Cursor::new(&mut extra[..]);
            try!(extra_buf.write_u32::<BigEndian>(flags));
            try!(extra_buf.write_u32::<BigEndian>(expiration));
        }

        let req_header = RequestHeader::from_payload(Command::AddQuietly, DataType::RawBytes, 0, opaque, 0,
                                                     key, &extra, value);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &extra,
                                key,
                                value);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        Ok(())
    }

    fn delete_noreply(&mut self, key: &[u8]) -> MemCachedResult<()> {
        let opaque = random::<u32>();
        debug!("Delete noreply key: {:?} {:?}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"));
        let req_header = RequestHeader::from_payload(Command::DeleteQuietly, DataType::RawBytes, 0, opaque, 0,
                                                     key, &[], &[]);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &[],
                                key,
                                &[]);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        Ok(())
    }

    fn replace_noreply(&mut self, key: &[u8], value: &[u8], flags: u32, expiration: u32) -> MemCachedResult<()> {
        let opaque = random::<u32>();
        debug!("Replace noreply key: {:?} {:?}, value: {:?}, flags: 0x{:x}, expiration: {}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), value, flags, expiration);
        let mut extra = [0u8; 8];
        {
            let mut extra_buf = Cursor::new(&mut extra[..]);
            try!(extra_buf.write_u32::<BigEndian>(flags));
            try!(extra_buf.write_u32::<BigEndian>(expiration));
        }

        let req_header = RequestHeader::from_payload(Command::ReplaceQuietly, DataType::RawBytes, 0, opaque, 0,
                                                     key, &extra, value);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &extra,
                                key,
                                value);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        Ok(())
    }

    fn increment_noreply(&mut self, key: &[u8], amount: u64, initial: u64, expiration: u32) -> MemCachedResult<()> {
        let opaque = random::<u32>();
        debug!("Increment noreply key: {:?} {:?}, amount: {}, initial: {}, expiration: {}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), amount, initial, expiration);
        let mut extra = [0u8; 20];
        {
            let mut extra_buf = Cursor::new(&mut extra[..]);
            try!(extra_buf.write_u64::<BigEndian>(amount));
            try!(extra_buf.write_u64::<BigEndian>(initial));
            try!(extra_buf.write_u32::<BigEndian>(expiration));
        }

        let req_header = RequestHeader::from_payload(Command::IncrementQuietly, DataType::RawBytes, 0, opaque, 0,
                                                     key, &extra, &[]);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &extra,
                                key,
                                &[]);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        Ok(())
    }

    fn decrement_noreply(&mut self, key: &[u8], amount: u64, initial: u64, expiration: u32) -> MemCachedResult<()> {
        let opaque = random::<u32>();
        debug!("Decrement noreply key: {:?} {:?}, amount: {}, initial: {}, expiration: {}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), amount, initial, expiration);
        let mut extra = [0u8; 20];
        {
            let mut extra_buf = Cursor::new(&mut extra[..]);
            try!(extra_buf.write_u64::<BigEndian>(amount));
            try!(extra_buf.write_u64::<BigEndian>(initial));
            try!(extra_buf.write_u32::<BigEndian>(expiration));
        }

        let req_header = RequestHeader::from_payload(Command::DecrementQuietly, DataType::RawBytes, 0, opaque, 0,
                                                     key, &extra, &[]);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &extra,
                                key,
                                &[]);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        Ok(())
    }

    fn append_noreply(&mut self, key: &[u8], value: &[u8]) -> MemCachedResult<()> {
        let opaque = random::<u32>();
        debug!("Append noreply key: {:?} {:?}, value: {:?}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), value);
        let req_header = RequestHeader::from_payload(Command::AppendQuietly, DataType::RawBytes, 0, opaque, 0,
                                                     key, &[], value);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &[],
                                key,
                                value);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        Ok(())
    }

    fn prepend_noreply(&mut self, key: &[u8], value: &[u8]) -> MemCachedResult<()> {
        let opaque = random::<u32>();
        debug!("Prepend noreply key: {:?} {:?}, value: {:?}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), value);
        let req_header = RequestHeader::from_payload(Command::PrependQuietly, DataType::RawBytes, 0, opaque, 0,
                                                     key, &[], value);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &[],
                                key,
                                value);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        Ok(())
    }
}

impl<T: BufRead + Write + Send> CasOperation for BinaryProto<T> {
    fn set_cas(&mut self, key: &[u8], value: &[u8], flags: u32, expiration: u32, cas: u64) -> MemCachedResult<u64> {
        let opaque = random::<u32>();
        debug!("Set cas key: {:?} {:?}, value: {:?}, flags: 0x{:x}, expiration: {}, cas: {}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), value, flags, expiration, cas);
        let mut extra = [0u8; 8];
        {
            let mut extra_buf = Cursor::new(&mut extra[..]);
            try!(extra_buf.write_u32::<BigEndian>(flags));
            try!(extra_buf.write_u32::<BigEndian>(expiration));
        }

        let req_header = RequestHeader::from_payload(Command::Set, DataType::RawBytes, 0, opaque, cas,
                                                     key, &extra, value);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &extra,
                                key,
                                value);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => Ok(resp.header.cas),
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn add_cas(&mut self, key: &[u8], value: &[u8], flags: u32, expiration: u32) -> MemCachedResult<u64> {
        let opaque = random::<u32>();
        debug!("Add cas key: {:?} {:?}, value: {:?}, flags: 0x{:x}, expiration: {}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), value, flags, expiration);
        let mut extra = [0u8; 8];
        {
            let mut extra_buf = Cursor::new(&mut extra[..]);
            try!(extra_buf.write_u32::<BigEndian>(flags));
            try!(extra_buf.write_u32::<BigEndian>(expiration));
        }

        let req_header = RequestHeader::from_payload(Command::Add, DataType::RawBytes, 0, opaque, 0,
                                                     key, &extra, value);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &extra,
                                key,
                                value);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => Ok(resp.header.cas),
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn replace_cas(&mut self, key: &[u8], value: &[u8], flags: u32, expiration: u32, cas: u64) -> MemCachedResult<u64> {
        let opaque = random::<u32>();
        debug!("Replace cas key: {:?} {:?}, value: {:?}, flags: 0x{:x}, expiration: {}, cas: {}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), value, flags, expiration, cas);
        let mut extra = [0u8; 8];
        {
            let mut extra_buf = Cursor::new(&mut extra[..]);
            try!(extra_buf.write_u32::<BigEndian>(flags));
            try!(extra_buf.write_u32::<BigEndian>(expiration));
        }

        let req_header = RequestHeader::from_payload(Command::Replace, DataType::RawBytes, 0, opaque, cas,
                                                     key, &extra, value);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &extra,
                                key,
                                value);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => Ok(resp.header.cas),
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn get_cas(&mut self, key: &[u8]) -> MemCachedResult<(Vec<u8>, u32, u64)> {
        let opaque = random::<u32>();
        debug!("Get cas key: {:?} {:?}", key, str::from_utf8(key).unwrap_or("<not-utf8-key>"));
        let req_header = RequestHeader::from_payload(Command::Get, DataType::RawBytes, 0, opaque, 0,
                                                     key, &[], &[]);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &[],
                                key,
                                &[]);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => {
                let mut extrabufr = BufReader::new(&resp.extra[..]);
                let flags = try!(extrabufr.read_u32::<BigEndian>());

                Ok((resp.value, flags, resp.header.cas))
            },
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn getk_cas(&mut self, key: &[u8]) -> MemCachedResult<(Vec<u8>, Vec<u8>, u32, u64)> {
        let opaque = random::<u32>();
        debug!("GetK cas key: {:?} {:?}", key, str::from_utf8(key).unwrap_or("<not-utf8-key>"));
        let req_header = RequestHeader::from_payload(Command::GetKey, DataType::RawBytes, 0, opaque, 0,
                                                     key, &[], &[]);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &[],
                                key,
                                &[]);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => {
                let mut extrabufr = BufReader::new(&resp.extra[..]);
                let flags = try!(extrabufr.read_u32::<BigEndian>());

                Ok((resp.key, resp.value, flags, resp.header.cas))
            },
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn increment_cas(&mut self, key: &[u8], amount: u64, initial: u64, expiration: u32, cas: u64)
            -> MemCachedResult<(u64, u64)> {
        let opaque = random::<u32>();
        debug!("Increment cas key: {:?} {:?}, amount: {}, initial: {}, expiration: {}, cas: {}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), amount, initial, expiration, cas);
        let mut extra = [0u8; 20];
        {
            let mut extra_buf = Cursor::new(&mut extra[..]);
            try!(extra_buf.write_u64::<BigEndian>(amount));
            try!(extra_buf.write_u64::<BigEndian>(initial));
            try!(extra_buf.write_u32::<BigEndian>(expiration));
        }

        let req_header = RequestHeader::from_payload(Command::Increment, DataType::RawBytes, 0, opaque, cas,
                                                     key, &extra, &[]);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &extra,
                                key,
                                &[]);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => {
                let mut bufr = BufReader::new(&resp.value[..]);
                Ok((try!(bufr.read_u64::<BigEndian>()), resp.header.cas))
            },
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn decrement_cas(&mut self, key: &[u8], amount: u64, initial: u64, expiration: u32, cas: u64)
            -> MemCachedResult<(u64, u64)> {
        let opaque = random::<u32>();
        debug!("Decrement cas key: {:?} {:?}, amount: {}, initial: {}, expiration: {}, cas: {}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), amount, initial, expiration, cas);
        let mut extra = [0u8; 20];
        {
            let mut extra_buf = Cursor::new(&mut extra[..]);
            try!(extra_buf.write_u64::<BigEndian>(amount));
            try!(extra_buf.write_u64::<BigEndian>(initial));
            try!(extra_buf.write_u32::<BigEndian>(expiration));
        }

        let req_header = RequestHeader::from_payload(Command::Decrement, DataType::RawBytes, 0, opaque, cas,
                                                     key, &extra, &[]);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &extra,
                                key,
                                &[]);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => {
                let mut bufr = BufReader::new(&resp.value[..]);
                Ok((try!(bufr.read_u64::<BigEndian>()), resp.header.cas))
            },
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn append_cas(&mut self, key: &[u8], value: &[u8], cas: u64) -> MemCachedResult<u64> {
        let opaque = random::<u32>();
        debug!("Append cas key: {:?} {:?}, value: {:?}, cas: {}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), value, cas);
        let req_header = RequestHeader::from_payload(Command::Append, DataType::RawBytes, 0, opaque, cas,
                                                     key, &[], value);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &[],
                                key,
                                value);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => Ok(resp.header.cas),
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn prepend_cas(&mut self, key: &[u8], value: &[u8], cas: u64) -> MemCachedResult<u64> {
        let opaque = random::<u32>();
        debug!("Prepend cas key: {:?} {:?}, value: {:?}, cas: {}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), value, cas);
        let req_header = RequestHeader::from_payload(Command::Prepend, DataType::RawBytes, 0, opaque, cas,
                                                     key, &[], value);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &[],
                                key,
                                value);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => Ok(resp.header.cas),
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn touch_cas(&mut self, key: &[u8], expiration: u32, cas: u64) -> MemCachedResult<u64> {
        let opaque = random::<u32>();
        debug!("Touch cas key: {:?} {:?}, expiration: {:?}, cas: {}",
               key, str::from_utf8(key).unwrap_or("<not-utf8-key>"), expiration, cas);
        let mut extra = [0u8; 4];
        {
            let mut extra_buf = Cursor::new(&mut extra[..]);
            try!(extra_buf.write_u32::<BigEndian>(expiration));
        }

        let req_header = RequestHeader::from_payload(Command::Touch, DataType::RawBytes, 0, opaque, cas,
                                                     key, &extra, &[]);
        let req_packet = RequestPacketRef::new(
                                &req_header,
                                &extra,
                                key,
                                &[]);

        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => Ok(resp.header.cas),
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }
}

impl<T: BufRead + Write + Send> AuthOperation for BinaryProto<T> {
    fn list_mechanisms(&mut self) -> MemCachedResult<Vec<String>> {
        let opaque = random::<u32>();
        debug!("List mechanisms");
        let req_header = RequestHeader::new(Command::SaslListMechanisms, DataType::RawBytes, 0, opaque, 0, 0, 0, 0);
        let req_packet = RequestPacketRef::new(&req_header, &[], &[], &[]);
        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::NoError => {},
            _ => return Err(From::from(Error::from_status(resp.header.status, None))),
        }

        match str::from_utf8(&resp.value[..]) {
            Ok(s) => {
                Ok(s.split(' ').map(|mech| mech.to_string()).collect())
            },
            Err(..) => {
                Err(proto::Error::OtherError {
                        desc: "Mechanism decode error",
                        detail: None,
                    })
            }
        }
    }

    fn auth_start(&mut self, mech: &str, init: &[u8]) -> MemCachedResult<AuthResponse> {
        let opaque = random::<u32>();
        debug!("Auth start, mechanism: {:?}, init: {:?}", mech, init);
        let req_header = RequestHeader::from_payload(Command::SaslAuthenticate, DataType::RawBytes, 0, opaque, 0,
                                                     mech.as_bytes(), &[], init);
        let req_packet = RequestPacketRef::new(&req_header, &[], mech.as_bytes(), init);
        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::AuthenticationFurtherStepRequired => Ok(AuthResponse::Continue(resp.value)),
            Status::NoError => Ok(AuthResponse::Succeeded),
            Status::AuthenticationRequired => Ok(AuthResponse::Failed),
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }

    fn auth_continue(&mut self, mech: &str, data: &[u8]) -> MemCachedResult<AuthResponse> {
        let opaque = random::<u32>();
        debug!("Auth continue, mechanism: {:?}, data: {:?}", mech, data);
        let req_header = RequestHeader::from_payload(Command::SaslStep, DataType::RawBytes, 0, opaque, 0,
                                                     mech.as_bytes(), &[], data);
        let req_packet = RequestPacketRef::new(&req_header, &[], mech.as_bytes(), data);
        try!(req_packet.write_to(&mut self.stream));
        try!(self.stream.flush());

        let mut resp = try!(ResponsePacket::read_from(&mut self.stream));
        while resp.header.opaque != opaque {
            debug!("Expecting opaque: {} but got {}, trying again ...", opaque, resp.header.opaque);
            resp = try!(ResponsePacket::read_from(&mut self.stream));
        }

        match resp.header.status {
            Status::AuthenticationFurtherStepRequired => Ok(AuthResponse::Continue(resp.value)),
            Status::NoError => Ok(AuthResponse::Succeeded),
            Status::AuthenticationRequired => Ok(AuthResponse::Failed),
            _ => Err(From::from(Error::from_status(resp.header.status, None))),
        }
    }
}

#[cfg(test)]
mod test {
    use std::net::TcpStream;
    use std::collections::{BTreeMap, HashMap};
    use proto::{Operation, MultiOperation, ServerOperation, NoReplyOperation,
                CasOperation, BinaryProto};

    use bufstream::BufStream;

    const SERVER_ADDR: &'static str = "127.0.0.1:11211";

    fn get_client() -> BinaryProto<BufStream<TcpStream>> {
        let stream = TcpStream::connect(SERVER_ADDR).unwrap();
        BinaryProto::new(BufStream::new(stream))
    }

    #[test]
    fn test_set_get_delete() {
        const KEY: &'static [u8] = b"test:set_get_delete";
        const VAL: &'static [u8] = b"world";

        let mut client = get_client();
        assert!(client.set(KEY, VAL, 0xdeadbeef, 120).is_ok());

        let get_resp = client.get(KEY);
        assert!(get_resp.is_ok());
        assert_eq!(get_resp.unwrap(), (VAL.to_vec(), 0xdeadbeef));

        let getk_resp = client.getk(KEY);
        assert!(getk_resp.is_ok());
        assert_eq!(getk_resp.unwrap(), (KEY.to_vec(), VAL.to_vec(), 0xdeadbeef));

        assert!(client.delete(KEY).is_ok());
    }

    #[test]
    fn test_incr_decr() {
        const KEY: &'static [u8] = b"test:incr_decr";

        let mut client = get_client();
        {
            let incr_resp = client.increment(KEY, 1, 0, 120);
            assert!(incr_resp.is_ok());
            assert_eq!(incr_resp.unwrap(), 0);
        }

        {
            let incr_resp = client.increment(KEY, 10, 0, 120);
            assert!(incr_resp.is_ok());
            assert_eq!(incr_resp.unwrap(), 10);
        }

        {
            let decr_resp = client.decrement(KEY, 5, 0, 120);
            assert!(decr_resp.is_ok());
            assert_eq!(decr_resp.unwrap(), 5);
        }

        {
            let decr_resp = client.decrement(KEY, 20, 0, 120);
            assert!(decr_resp.is_ok());
            assert_eq!(decr_resp.unwrap(), 0);
        }

        assert!(client.delete(KEY).is_ok())
    }

    #[test]
    fn test_version() {
        let mut client = get_client();
        assert!(client.version().is_ok());
    }

    #[test]
    fn test_noop() {
        let mut client = get_client();
        assert!(client.noop().is_ok());
    }

    #[test]
    #[should_panic]
    fn test_quit() {
        let mut client = get_client();
        assert!(client.quit().is_ok());

        client.noop().unwrap();
    }

    #[test]
    fn test_flush() {
        let mut client = get_client();
        assert!(client.flush(2).is_ok());
    }

    #[test]
    fn test_add() {
        const KEY: &'static [u8] = b"test:add";
        const INIT_VAL: &'static [u8] = b"initial";
        const ADD_VAL: &'static [u8] = b"added";

        let mut client = get_client();

        {
            let add_resp = client.add(KEY, INIT_VAL, 0xdeadbeef, 120);
            assert!(add_resp.is_ok(), "{:?}", add_resp);
        }

        {
            let get_resp = client.get(KEY);
            assert!(get_resp.is_ok(), "{:?}", get_resp);

            assert_eq!(get_resp.unwrap(), (INIT_VAL.to_vec(), 0xdeadbeef));
            let add_resp = client.add(KEY, ADD_VAL, 0xdeadbeef, 120);
            assert!(add_resp.is_err(), "{:?}", add_resp);
        }

        assert!(client.delete(KEY).is_ok());
    }

    #[test]
    fn test_replace() {
        let mut client = get_client();

        {
            let rep_resp = client.replace(b"test:replace_key", b"replaced", 0xdeadbeef, 120);
            assert!(rep_resp.is_err());
        }

        {
            let add_resp = client.add(b"test:replace_key", b"just_add", 0xdeadbeef, 120);
            assert!(add_resp.is_ok());
            let rep_resp = client.replace(b"test:replace_key", b"replaced", 0xdeadbeef, 120);
            assert!(rep_resp.is_ok());
            assert!(client.delete(b"test:replace_key").is_ok());
        }
    }

    #[test]
    fn test_append_prepend() {
        let mut client = get_client();
        {
            let app_resp = client.append(b"test:append_key", b"appended");
            assert!(app_resp.is_err());
            let pre_resp = client.prepend(b"test:append_key", b"prepended");
            assert!(pre_resp.is_err());
        }

        {
            let add_resp = client.add(b"test:append_key", b"just_add", 0xdeadbeef, 120);
            assert!(add_resp.is_ok());

            let app_resp = client.append(b"test:append_key", b"appended");
            assert!(app_resp.is_ok());
            let get_resp = client.get(b"test:append_key");
            assert!(get_resp.is_ok());
            assert_eq!(get_resp.unwrap(), (b"just_addappended".to_vec(), 0xdeadbeef));

            let pre_resp = client.prepend(b"test:append_key", b"prepended");
            assert!(pre_resp.is_ok());
            let get_resp = client.get(b"test:append_key");
            assert!(get_resp.is_ok());
            assert_eq!(get_resp.unwrap(), (b"prependedjust_addappended".to_vec(), 0xdeadbeef));
        }

        assert!(client.delete(b"test:append_key").is_ok());
    }

    #[test]
    fn test_stat() {
        let mut client = get_client();
        let stat_resp = client.stat();
        assert!(stat_resp.is_ok());
    }

    #[test]
    fn test_touch() {
        let mut client = get_client();

        let touch_resp = client.touch(b"test:touch", 120);
        assert!(touch_resp.is_err());

        let add_resp = client.add(b"test:touch", b"val", 0xcafebabe, 100);
        assert!(add_resp.is_ok());

        let touch_resp = client.touch(b"test:touch", 120);
        assert!(touch_resp.is_ok());

        assert!(client.delete(b"test:touch").is_ok());
    }

    #[test]
    fn test_set_get_delete_incr_muti() {
        let mut client = get_client();

        let mut data = BTreeMap::new();
        data.insert(&b"test:multi_hello1"[..], (&b"world1"[..], 0xdeadbeef, 120));
        data.insert(&b"test:multi_hello2"[..], (&b"world2"[..], 0xdeadbeef, 120));
        data.insert(&b"test:multi_num1"[..], (&b"100"[..], 0xdeadbeef, 120));
        data.insert(&b"test:multi_num2"[..], (&b"200"[..], 0xdeadbeef, 120));
        data.insert(&b"test:multi_lastone"[..], (&b"last!"[..], 0xdeadbeef, 120));

        let set_resp = client.set_multi(data);
        assert!(set_resp.is_ok());

        let get_resp = client.get_multi(&[b"test:multi_hello1",
                                          b"test:multi_hello2",
                                          b"test:multi_lastone"]);
        assert!(get_resp.is_ok());

        let get_resp_map = get_resp.as_ref().unwrap();
        assert_eq!(get_resp_map.get(&b"test:multi_hello1".to_vec()),
                   Some(&(b"world1".to_vec(), 0xdeadbeef)));
        assert_eq!(get_resp_map.get(&b"test:multi_hello2".to_vec()),
                   Some(&(b"world2".to_vec(), 0xdeadbeef)));
        assert_eq!(get_resp_map.get(&b"test:multi_lastone".to_vec()),
                   Some(&(b"last!".to_vec(), 0xdeadbeef)));

        let del_resp = client.delete_multi(&[b"test:multi_hello1",
                                             b"test:multi_hello2"]);
        assert!(del_resp.is_ok());

        let get_resp = client.get_multi(&[b"test:multi_hello1",
                                          b"test:multi_hello2",
                                          b"test:multi_lastone"]);
        assert!(get_resp.is_ok());

        let get_resp_map = get_resp.as_ref().unwrap();
        assert_eq!(get_resp_map.get(&b"test:multi_hello1".to_vec()),
                   None);
        assert_eq!(get_resp_map.get(&b"test:multi_hello2".to_vec()),
                   None);
        assert_eq!(get_resp_map.get(&b"test:multi_lastone".to_vec()),
                   Some(&(b"last!".to_vec(), 0xdeadbeef)));

        let mut data = HashMap::new();
        data.insert(&b"test:multi_num1"[..], (10, 50, 120));
        data.insert(&b"test:multi_num2"[..], (20, 50, 120));
        data.insert(&b"test:multi_num3"[..], (30, 50, 120));
        let incr_resp = client.increment_multi(data);
        assert!(incr_resp.is_ok());

        let get_resp = client.get_multi(&[b"test:multi_num1",
                                          b"test:multi_num2",
                                          b"test:multi_num3"]);
        assert!(get_resp.is_ok());

        let get_resp_map = get_resp.as_ref().unwrap();
        assert_eq!(get_resp_map.get(&b"test:multi_num1".to_vec()),
                   Some(&(b"110".to_vec(), 0xdeadbeef)));
        assert_eq!(get_resp_map.get(&b"test:multi_num2".to_vec()),
                   Some(&(b"220".to_vec(), 0xdeadbeef)));
        assert_eq!(get_resp_map.get(&b"test:multi_num3".to_vec()),
                   Some(&(b"50".to_vec(), 0x0)));

        let del_resp = client.delete_multi(&[b"lastone",
                                             b"not_exists!!!!"]);
        assert!(del_resp.is_ok());
    }

    #[test]
    fn test_set_add_replace_noreply() {
        let key = b"test:noreply_key";
        let set_val = b"value";
        let add_val = b"just add";
        let rep_val = b"replaced";

        let mut client = get_client();

        let add_resp = client.add_noreply(key, add_val, 0xdeadbeef, 120);
        assert!(add_resp.is_ok());

        let get_resp = client.get(key);
        assert!(get_resp.is_ok());
        assert_eq!(get_resp.unwrap(), (add_val.to_vec(), 0xdeadbeef));

        let set_resp = client.set_noreply(key, set_val, 0xdeadbeef, 120);
        assert!(set_resp.is_ok());

        let get_resp = client.get(key);
        assert!(get_resp.is_ok());
        assert_eq!(get_resp.unwrap(), (set_val.to_vec(), 0xdeadbeef));

        let rep_resp = client.replace_noreply(key, rep_val, 0xcafebabe, 120);
        assert!(rep_resp.is_ok());

        let get_resp = client.get(key);
        assert!(get_resp.is_ok());
        assert_eq!(get_resp.unwrap(), (rep_val.to_vec(), 0xcafebabe));

        assert!(client.delete(key).is_ok());
    }

    #[test]
    fn test_set_add_replace_cas() {
        let key = b"test:cas_key";
        let set_val = b"value";
        let add_val = b"just add";
        let rep_val = b"replaced";

        let mut client = get_client();

        let add_resp = client.add_cas(key, add_val, 0xdeadbeef, 120);
        assert!(add_resp.is_ok());
        let add_cas = add_resp.unwrap();

        {
            let set_resp = client.set_cas(key, set_val, 0xdeadbeef, 120, add_cas + 1);
            assert!(set_resp.is_err());

            let get_resp = client.get_cas(key);
            assert!(get_resp.is_ok());
            let (_, _, get_cas) = get_resp.unwrap();
            assert_eq!(get_cas, add_cas);

            let rep_resp = client.replace_cas(key, rep_val, 0xdeadbeef, 120, add_cas + 1);
            assert!(rep_resp.is_err());
        }

        {
            let set_resp = client.set_cas(key, set_val, 0xdeadbeef, 120, add_cas);
            assert!(set_resp.is_ok());
            let set_cas = set_resp.unwrap();

            let get_resp = client.get_cas(key);
            assert!(get_resp.is_ok());
            let (_, _, get_cas) = get_resp.unwrap();
            assert_eq!(get_cas, set_cas);

            let rep_resp = client.replace_cas(key, rep_val, 0xdeadbeef, 120, set_cas);
            assert!(rep_resp.is_ok());
        }

        assert!(client.delete(key).is_ok());
    }

    #[test]
    fn test_incr_decr_cas() {
        let key = b"test:incr_decr_cas";
        let mut client = get_client();

        let incr_resp = client.increment_cas(key, 0, 100, 120, 0);
        assert!(incr_resp.is_ok());
        let (_, incr_cas) = incr_resp.unwrap();

        let incr_resp = client.increment_cas(key, 0, 10, 120, incr_cas + 1);
        assert!(incr_resp.is_err());

        let incr_resp = client.increment_cas(key, 0, 10, 120, incr_cas);
        assert!(incr_resp.is_ok());
        let (_, incr_cas) = incr_resp.unwrap();

        let decr_resp = client.decrement_cas(key, 0, 10, 120, incr_cas + 1);
        assert!(decr_resp.is_err());

        let decr_resp = client.decrement_cas(key, 0, 10, 120, incr_cas);
        assert!(decr_resp.is_ok());

        assert!(client.delete(key).is_ok());
    }

    #[test]
    fn test_append_prepend_cas() {
        const KEY: &'static [u8] = b"test:append_prepend_cas";
        let mut client = get_client();

        let set_resp = client.set_cas(KEY, b"appended", 0, 120, 0);
        assert!(set_resp.is_ok());
        let set_cas = set_resp.unwrap();

        let ap_resp = client.append_cas(KEY, b"appended", set_cas + 1);
        assert!(ap_resp.is_err());

        let ap_resp = client.append_cas(KEY, b"appended", set_cas);
        assert!(ap_resp.is_ok());
        let ap_cas = ap_resp.unwrap();

        let pr_resp = client.prepend_cas(KEY, b"prepend", ap_cas + 1);
        assert!(pr_resp.is_err());

        let pr_resp = client.prepend_cas(KEY, b"prepend", ap_cas);
        assert!(pr_resp.is_ok());

        assert!(client.delete(KEY).is_ok());
    }

    #[test]
    fn test_if_noreply_failed() {
        let key = b"test:noreply_fail_key";
        let set_val = b"value";
        let add_val = b"just add";

        let mut client = get_client();

        let set_resp = client.set_noreply(key, set_val, 0xdeadbeef, 120);
        assert!(set_resp.is_ok());

        // Should failed, because key is already set
        let add_resp = client.add_noreply(key, add_val, 0xdeadbeef, 120);
        assert!(add_resp.is_ok());

        let get_resp = client.get(key);
        assert!(get_resp.is_ok());
        assert_eq!(get_resp.unwrap(), (set_val.to_vec(), 0xdeadbeef));
    }
}
