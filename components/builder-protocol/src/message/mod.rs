// Copyright (c) 2016-2017 Chef Software Inc. and/or applicable contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

pub mod jobsrv;
pub mod routesrv;
pub mod sessionsrv;
pub mod originsrv;
pub mod scheduler;
mod net;

use std::fmt;
use std::hash::Hasher;

use protobuf::{self, Clear};

pub use self::net::{ErrCode, NetError, NetOk, Protocol};
use error::ProtocolError;
use sharding::InstaId;

const MAX_BODY_LEN: usize = (128 * 1024) * 8;
const MAX_IDENTITIES: usize = 10;

#[derive(Debug, Default)]
pub struct Header(net::Header);

impl Header {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let inner = decode::<net::Header>(bytes)?;
        Ok(Header(inner))
    }

    pub fn message_id(&self) -> &str {
        self.0.get_message_id()
    }

    pub fn has_route_info(&self) -> bool {
        self.0.has_route_info()
    }

    pub fn has_txn(&self) -> bool {
        self.0.has_txn()
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, ProtocolError> {
        encode(&self.0)
    }
}

impl Clear for Header {
    fn clear(&mut self) {
        self.0.clear()
    }
}

#[derive(Debug)]
pub struct Message {
    /// Returns the binary representation of the body of the message.
    ///
    /// This can be parsed into a protocol message with `parse()`
    pub body: Vec<u8>,
    /// Ordered list of network identities of servers which have handled the message starting
    /// with the originator.
    pub identities: Vec<String>,
    /// Message buffer for `header` portion of a router message.
    header: Header,
    /// Message buffer for `route_info` portion of a router message.
    route_info: Option<RouteInfo>,
    /// Message buffer for `txn` portion of a router message.
    txn: Option<Txn>,
}

impl Message {
    /// Clear all fields for message instance.
    ///
    /// Useful if you want to re-use the Message struct without allocating a new one.
    pub fn reset(&mut self) {
        self.identities.clear();
        self.header.clear();
        self.txn = None;
        self.route_info = None;
        self.body.clear();
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn message_id(&self) -> &str {
        self.header.message_id()
    }

    pub fn parse<T>(&self) -> Result<T, ProtocolError>
    where
        T: protobuf::MessageStatic,
    {
        decode::<T>(&self.body)
    }

    pub fn route_info(&self) -> Option<&RouteInfo> {
        self.route_info.as_ref()
    }

    pub fn txn(&self) -> Option<&Txn> {
        self.txn.as_ref()
    }

    pub fn txn_mut(&mut self) -> Option<&mut Txn> {
        self.txn.as_mut()
    }

    pub fn sender(&self) -> Result<&str, ProtocolError> {
        self.identities.first().map(String::as_ref).ok_or(
            ProtocolError::MsgNotInitialized,
        )
    }

    pub fn push_identity(&mut self, bytes: Vec<u8>) -> Result<(), ProtocolError> {
        let identity = String::from_utf8(bytes).map_err(
            ProtocolError::IdentityDecode,
        )?;
        self.identities.push(identity);
        Ok(())
    }

    pub fn set_body<T>(&mut self, body: &T) -> Result<(), ProtocolError>
    where
        T: protobuf::Message,
    {
        self.body = encode::<T>(body)?;
        Ok(())
    }

    pub fn set_header(&mut self, bytes: &[u8]) -> Result<(), ProtocolError> {
        self.header = Header::from_bytes(bytes)?;
        Ok(())
    }

    pub fn set_routing(&mut self, bytes: &[u8]) -> Result<(), ProtocolError> {
        self.route_info = Some(RouteInfo::from_bytes(bytes)?);
        Ok(())
    }

    pub fn set_txn(&mut self, bytes: &[u8]) -> Result<(), ProtocolError> {
        self.txn = Some(Txn::from_bytes(bytes)?);
        Ok(())
    }
}

impl Default for Message {
    fn default() -> Self {
        Message {
            body: Vec::with_capacity(MAX_BODY_LEN),
            identities: Vec::with_capacity(MAX_IDENTITIES),
            header: Header::default(),
            route_info: None,
            txn: None,
        }
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut msg = format!("[{}]", self.message_id());
        if let Ok(ref sender) = self.sender() {
            msg.push_str(&format!(" {}", sender));
        }
        if let Some(ref route_info) = self.route_info {
            msg.push_str(&format!(", proto={}", route_info.protocol()));
            if let Some(ref hash) = route_info.hash() {
                msg.push_str(&format!(", hash={}", hash));
            }
        }
        if let Some(ref txn) = self.txn {
            msg.push_str(&format!(
                ", txn_id={}, txn_complete={}",
                txn.id(),
                txn.is_complete()
            ));
        }
        write!(f, "{}", msg)
    }
}

#[derive(Debug, Default)]
pub struct RouteInfo(net::RouteInfo);

impl RouteInfo {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let inner = decode::<net::RouteInfo>(bytes)?;
        Ok(RouteInfo(inner))
    }

    pub fn protocol(&self) -> net::Protocol {
        self.0.get_protocol()
    }

    pub fn hash(&self) -> Option<u64> {
        if self.0.has_hash() {
            Some(self.0.get_hash())
        } else {
            None
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, ProtocolError> {
        encode(&self.0)
    }
}

impl Clear for RouteInfo {
    fn clear(&mut self) {
        self.0.clear()
    }
}

#[derive(Debug, Default)]
pub struct Txn(net::Txn);

impl Txn {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let inner = decode::<net::Txn>(bytes)?;
        Ok(Txn(inner))
    }

    pub fn id(&self) -> u64 {
        self.0.get_id()
    }

    pub fn is_complete(&self) -> bool {
        self.0.get_complete()
    }

    pub fn set_complete(&mut self, value: bool) {
        self.0.set_complete(value);
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, ProtocolError> {
        encode(&self.0)
    }
}

impl Clear for Txn {
    fn clear(&mut self) {
        self.0.clear()
    }
}

/// Defines a contract for protocol messages to be persisted to a datastore.
pub trait Persistable: protobuf::Message + protobuf::MessageStatic {
    /// Type of the primary key
    type Key: fmt::Display;

    /// Returns the value of the primary key.
    fn primary_key(&self) -> Self::Key;

    /// Sets the primary key to the given value.
    fn set_primary_key(&mut self, value: Self::Key) -> ();
}

/// Defines a contract for protocol messages to be routed through `RouteSrv`.
pub trait Routable: protobuf::Message + protobuf::MessageStatic {
    /// Type of the route key
    type H: RouteKey + fmt::Display;

    /// Return a `RouteKey` for `RouteSrv` to know which key's value to route on.
    ///
    /// If `Some(T)`, the message will be routed by hashing the value of the route key and modding
    /// it against the shard count. This is known as "randomly deterministic routing".
    ///
    /// If `None`, the message will be randomly routed to an available node.
    fn route_key(&self) -> Option<Self::H>;
}

/// Provides an interface for hashing the implementing type for `Routable` messages.
///
/// Some types contain "hints" that help `RouteSrv` to identify the destination shard for a
/// message. You can leverage this trait to take any hints into account. See the implementation of
/// this trait for `InstaId` for an example.
pub trait RouteKey {
    /// Hashes a route key providing a route hash.
    fn hash(&self, hasher: &mut Hasher) -> u64;
}

impl RouteKey for String {
    fn hash(&self, hasher: &mut Hasher) -> u64 {
        hasher.write(self.as_bytes());
        hasher.finish()
    }
}

impl RouteKey for InstaId {
    fn hash(&self, _hasher: &mut Hasher) -> u64 {
        self.shard()
    }
}

impl RouteKey for u64 {
    fn hash(&self, _hasher: &mut Hasher) -> u64 {
        *self
    }
}

pub fn decode<T>(bytes: &[u8]) -> Result<T, ProtocolError>
where
    T: protobuf::MessageStatic,
{
    protobuf::parse_from_bytes::<T>(bytes).map_err(ProtocolError::Decode)
}

pub fn encode<T>(message: &T) -> Result<Vec<u8>, ProtocolError>
where
    T: protobuf::Message,
{
    message.write_to_bytes().map_err(ProtocolError::Encode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_protocol() {
        assert_eq!(
            Message(&jobsrv::Job::new()).protocol(),
            net::Protocol::JobSrv
        );
        assert_eq!(Message(&net::Ping::new()).protocol(), net::Protocol::Net);
        assert_eq!(
            Message(&routesrv::Connect::new()).protocol(),
            net::Protocol::RouteSrv
        );
        assert_eq!(
            Message(&sessionsrv::Session::new()).protocol(),
            net::Protocol::SessionSrv
        );
        assert_eq!(
            Message(&originsrv::Origin::new()).protocol(),
            net::Protocol::OriginSrv
        );
        assert_eq!(
            Message(&scheduler::GroupCreate::new()).protocol(),
            net::Protocol::Scheduler
        );
    }
}
