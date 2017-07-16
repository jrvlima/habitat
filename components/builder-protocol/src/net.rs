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

use std::error;
use std::fmt;
use std::result;
use std::str::FromStr;

use protobuf::core::ProtobufEnum;
use serde::{Serialize, Serializer};
use serde::ser::SerializeStruct;

pub use message::{ErrCode, Message, NetError, NetOk, Protocol, RouteInfo, Txn};
use error::ProtocolError;

pub fn err<M>(code: ErrCode, msg: M) -> NetError
where
    M: ToString,
{
    let mut err = NetError::new();
    err.set_code(code);
    err.set_msg(msg.to_string());
    err
}

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[err: {:?}, msg: {}]", self.get_code(), self.get_msg())
    }
}

impl error::Error for NetError {
    fn description(&self) -> &str {
        match self.get_code() {
            ErrCode::BUG => "An unexpected error occurred.",
            ErrCode::TIMEOUT => "Network timeout.",
            ErrCode::REMOTE_REJECTED => "Remote server rejected request.",
            ErrCode::BAD_REMOTE_REPLY => "Remote server returned a bad response.",
            ErrCode::ENTITY_NOT_FOUND => "Entity not found in datastore.",
            ErrCode::NO_SHARD => "Shard not available.",
            ErrCode::ACCESS_DENIED => "Operation not allowed by authenticated.",
            ErrCode::SESSION_EXPIRED => "Session expired, user should re-authenticate.",
            ErrCode::ENTITY_CONFLICT => "Entity already exists in datastore.",
            ErrCode::ZMQ => "Network error.",
            ErrCode::DATA_STORE => "Database error.",
            ErrCode::AUTH_SCOPE => "Additional authorization scope(s) required for action.",
            ErrCode::WORKSPACE_SETUP => "Worker runner unable to setup build workspace.",
            ErrCode::SECRET_KEY_FETCH => "Worker runner unable to fetch secret key for origin.",
            ErrCode::SECRET_KEY_IMPORT => "Worker runner unable to import secret key for origin.",
            ErrCode::VCS_CLONE => "Worker runner unable to retrieve project source to build.",
            ErrCode::BUILD => "Worker runner failed to build project.",
            ErrCode::POST_PROCESSOR => "One or more post processing step failed in Worker runner.",
            ErrCode::REG_CONFLICT => {
                "Service registration rejected by RouteSrv. Conflicting registration."
            }
        }
    }
}

impl Serialize for ErrCode {
    fn serialize<S>(&self, serializer: S) -> result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(self.value() as u64)
    }
}

impl Serialize for NetError {
    fn serialize<S>(&self, serializer: S) -> result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut strukt = serializer.serialize_struct("error", 2)?;
        strukt.serialize_field("code", &self.get_code())?;
        strukt.serialize_field("msg", self.get_msg())?;
        strukt.end()
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // If you add a new value here, you *must* update the `FromStr` implementation for
        // `Protocol` below.
        let value = match *self {
            Protocol::JobSrv => "jobsrv",
            Protocol::Net => "net",
            Protocol::RouteSrv => "routesrv",
            Protocol::SessionSrv => "sessionsrv",
            Protocol::OriginSrv => "originsrv",
            Protocol::Scheduler => "scheduler",
        };
        write!(f, "{}", value)
    }
}

impl FromStr for Protocol {
    type Err = ProtocolError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "jobsrv" => Ok(Protocol::JobSrv),
            "net" => Ok(Protocol::Net),
            "routesrv" => Ok(Protocol::RouteSrv),
            "sessionsrv" => Ok(Protocol::SessionSrv),
            "originsrv" => Ok(Protocol::OriginSrv),
            "scheduler" => Ok(Protocol::Scheduler),
            protocol_id => Err(ProtocolError::NoProtocol(protocol_id.to_string())),
        }
    }
}
