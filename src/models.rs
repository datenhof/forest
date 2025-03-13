use std::fmt::Display;
use serde::{Serialize,Deserialize};

#[derive(Debug, Clone, PartialEq)]
pub enum DefaultString {
    Default,
    Custom(String),
}

impl Serialize for DefaultString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            DefaultString::Default => serializer.serialize_str("default"),
            DefaultString::Custom(name) => serializer.serialize_str(name),
        }
    }
}

impl<'de> Deserialize<'de> for DefaultString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(if s.to_lowercase() == "default" {
            DefaultString::Default
        } else {
            DefaultString::Custom(s)
        })
    }
}

impl Display for DefaultString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DefaultString::Default => write!(f, "default"),
            DefaultString::Custom(name) => write!(f, "{}", name),
        }
    }
}

impl DefaultString {
    pub fn new(name: &str) -> Self {
        if name.to_lowercase() == "default" {
            DefaultString::Default
        } else {
            DefaultString::Custom(name.to_string())
        }
    }

    pub fn from_str(s: &str) -> Self {
        Self::new(s)
    }

    pub fn from_option(s: Option<&str>) -> Self {
        match s {
            Some(s) => Self::new(s),
            None => DefaultString::Default,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            DefaultString::Default => "default",
            DefaultString::Custom(name) => name,
        }
    }
}

pub type ShadowName = DefaultString;
pub type TenantId = DefaultString;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceMetadata {
    pub device_id: String,
    pub tenant_id: TenantId,
    pub certificate: Option<String>,
    pub key: Option<String>,
    pub created_at: u64,
}

// Add this struct to your models.rs file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInformation {
    pub device_id: String,
    pub tenant_id: TenantId,
    pub certificate: Option<String>,
    pub connected: bool,
    pub last_shadow_update: Option<u64>,
}

impl DeviceMetadata {
    pub fn new(device_id: &str, tenant_id: &TenantId) -> Self {
        Self {
            device_id: device_id.to_string(),
            tenant_id: tenant_id.to_owned(),
            certificate: None,
            key: None,
            created_at: chrono::Utc::now().timestamp() as u64,
        }
    }
    
    pub fn with_credentials(mut self, certificate: String, key: String) -> Self {
        self.certificate = Some(certificate);
        self.key = Some(key);
        self
    }
}