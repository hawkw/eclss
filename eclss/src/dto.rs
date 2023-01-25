use edge_frame::dto::Role;
use embedded_svc::wifi::ClientConfiguration;
use serde::{Deserialize, Serialize};

pub const USERNAME_MAX_LEN: usize = 32;
pub const PASSWORD_MAX_LEN: usize = 32;

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum WebRequest {
    Authenticate(String, String),
    Logout,
    WifiSettings(ClientConfiguration),
    // TODO(eliza): calibration
    Calibrate,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum WebEvent {
    NoPermissions,

    AuthenticationFailed,

    RoleState(Role),

    // TODO(eliza): calibration
    SensorState(()),

    WifiState(ClientConfiguration),
}

// === impl WebRequest ==

impl WebRequest {
    pub fn role(&self) -> Role {
        match self {
            Self::Authenticate(_, _) => Role::None,
            Self::Logout => Role::None,
            Self::WifiSettings(_) => Role::Admin,
            Self::Calibrate => Role::Admin,
        }
    }
}

// === impl WebEvent ==

impl WebEvent {
    pub fn role(&self) -> Role {
        match self {
            Self::NoPermissions => Role::None,
            Self::AuthenticationFailed => Role::None,
            Self::RoleState(_) => Role::None,
            Self::WifiState(_) => Role::User,
            Self::SensorState(_) => Role::User,
        }
    }
}
