use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::{ShadowName, TenantId};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ShadowError {
    #[error("DeviceId mismatch")]
    DeviceIdMismatch,
    #[error("ShadowName mismatch")]
    ShadowNameMismatch,
    #[error("TenantId mismatch")]
    TenantIdMismatch,
}

#[derive(Error, Debug)]
pub enum ShadowSerializationError {
    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shadow {
    pub device_id: String,
    pub shadow_name: ShadowName,
    pub tenant_id: TenantId,
    state: StateDocument,
    metadata: MetadataDocument,
    version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateUpdateDocument {
    pub device_id: String,
    pub shadow_name: ShadowName,
    pub tenant_id: TenantId,
    pub state: StateDocument,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDocument {
    #[serde(skip_serializing_if = "Value::is_null")]
    #[serde(default)]
    pub reported: Value,
    #[serde(skip_serializing_if = "Value::is_null")]
    #[serde(default)]
    pub desired: Value,
    #[serde(skip_serializing_if = "Value::is_null")]
    #[serde(default)]
    pub delta: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NestedStateDocument {
    pub state: StateDocument,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataDocument {
    pub reported: Value,
    pub desired: Value,
}

impl NestedStateDocument {
    pub fn from_json(json: &str) -> Result<Self, ShadowSerializationError> {
        Ok(serde_json::from_str(json)?)
    }
}

impl StateUpdateDocument {
    pub fn new(device_id: &str, shadow_name: &ShadowName, tenant_id: &TenantId) -> Self {
        StateUpdateDocument {
            device_id: device_id.to_string(),
            shadow_name: shadow_name.to_owned(),
            tenant_id: tenant_id.to_owned(),
            state: StateDocument {
                reported: Value::Null,
                desired: Value::Null,
                delta: Value::Null,
            },
        }
    }

    pub fn from_json(json: &str) -> Result<Self, ShadowSerializationError> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn from_nested_json(
        json: &str,
        device_id: &str,
        shadow_name: &ShadowName,
        tenant_id: &TenantId,
    ) -> Result<Self, ShadowSerializationError> {
        let nested: NestedStateDocument = serde_json::from_str(json)?;
        Ok(StateUpdateDocument::from_nested_state(
            nested,
            device_id,
            shadow_name,
            tenant_id,
        ))
    }

    pub fn from_nested_state(
        nested: NestedStateDocument,
        device_id: &str,
        shadow_name: &ShadowName,
        tenant_id: &TenantId,
    ) -> Self {
        StateUpdateDocument {
            device_id: device_id.to_string(),
            shadow_name: shadow_name.to_owned(),
            tenant_id: tenant_id.to_owned(),
            state: nested.state,
        }
    }

    pub fn to_json(&self) -> Result<String, ShadowSerializationError> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn to_json_pretty(&self) -> Result<String, ShadowSerializationError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn get_reported_value(&self) -> &Value {
        &self.state.reported
    }

    pub fn get_desired_value(&self) -> &Value {
        &self.state.desired
    }

    pub fn set_reported_value(&mut self, value: Value) {
        self.state.reported = value;
    }

    pub fn set_desired_value(&mut self, value: Value) {
        self.state.desired = value;
    }
}

impl Shadow {
    pub fn new(device_id: &str, shadow_name: &ShadowName, tenant_id: &TenantId) -> Self {
        Shadow {
            device_id: device_id.to_string(),
            shadow_name: shadow_name.to_owned(),
            tenant_id: tenant_id.to_owned(),
            state: StateDocument {
                reported: Value::Null,
                desired: Value::Null,
                delta: Value::Null,
            },
            metadata: MetadataDocument {
                reported: Value::Null,
                desired: Value::Null,
            },
            version: 0,
        }
    }

    pub fn from_json(json: &str) -> Result<Self, ShadowSerializationError> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn to_json(&self) -> Result<String, ShadowSerializationError> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn to_json_pretty(&self) -> Result<String, ShadowSerializationError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn get_reported_value(&self) -> &Value {
        &self.state.reported
    }

    pub fn get_desired_value(&self) -> &Value {
        &self.state.desired
    }

    pub fn get_delta_value(&self) -> &Value {
        &self.state.delta
    }

    pub fn get_delta_json(&self) -> Result<Option<String>, ShadowSerializationError> {
        if !self.state.delta.is_object() {
            return Ok(None);
        }
        let delta_obj = self.state.delta.as_object().unwrap();
        if delta_obj.is_empty() {
            return Ok(None);
        }
        Ok(Some(serde_json::to_string(delta_obj)?))
    }

    pub fn get_reported_metadata(&self) -> &Value {
        &self.metadata.reported
    }

    pub fn get_desired_metadata(&self) -> &Value {
        &self.metadata.desired
    }

    pub fn get_version(&self) -> u64 {
        self.version
    }

    fn calculate_delta(&mut self) {
        fn diff_recursive(reported: &Value, desired: &Value) -> Option<Value> {
            match (reported, desired) {
                (Value::Object(rep), Value::Object(des)) => {
                    let mut delta_obj = serde_json::Map::new();
                    for (key, des_val) in des {
                        match rep.get(key) {
                            Some(rep_val) => {
                                if rep_val != des_val {
                                    if let Some(diff) = diff_recursive(rep_val, des_val) {
                                        delta_obj.insert(key.clone(), diff);
                                    }
                                }
                            }
                            None => {
                                delta_obj.insert(key.clone(), des_val.clone());
                            }
                        }
                    }
                    if delta_obj.is_empty() {
                        None
                    } else {
                        Some(Value::Object(delta_obj))
                    }
                }
                (reported, desired) if reported != desired => Some(desired.clone()),
                _ => None,
            }
        }

        self.state.delta = match diff_recursive(&self.state.reported, &self.state.desired) {
            Some(delta) => delta,
            None => Value::Null,
        };
    }

    pub fn update(&mut self, update: &StateUpdateDocument) -> Result<(), ShadowError> {
        // Verify identity
        if self.device_id != update.device_id {
            return Err(ShadowError::DeviceIdMismatch);
        }
        if self.shadow_name != update.shadow_name {
            return Err(ShadowError::ShadowNameMismatch);
        }
        if self.tenant_id != update.tenant_id {
            return Err(ShadowError::TenantIdMismatch);
        }

        // Update state
        if !update.state.reported.is_null() || !update.state.desired.is_null() {
            self.state.update(&update.state, &mut self.metadata);
        }

        // Calculate delta and increment version
        self.calculate_delta();
        self.version += 1;

        Ok(())
    }
}

impl StateDocument {
    fn current_timestamp() -> u64 {
        Utc::now().timestamp() as u64
    }

    pub fn update(&mut self, update: &StateDocument, metadata: &mut MetadataDocument) {
        // Ensure metadata state starts as an object
        if metadata.reported.is_null() {
            metadata.reported = Value::Object(serde_json::Map::new());
        }

        if metadata.desired.is_null() {
            metadata.desired = Value::Object(serde_json::Map::new());
        }

        fn update_recursive(
            current: &mut Value,
            update: &Value,
            metadata_value: &mut Value,
            timestamp: u64,
        ) {
            match update {
                Value::Object(map) => {
                    // Ensure current and metadata are objects
                    if current.is_null() {
                        *current = Value::Object(serde_json::Map::new());
                    }
                    if metadata_value.is_null() {
                        *metadata_value = Value::Object(serde_json::Map::new());
                    }

                    if let Some(current_obj) = current.as_object_mut() {
                        let metadata_obj = metadata_value.as_object_mut().unwrap();
                        for (key, value) in map {
                            if value.is_null() {
                                current_obj.remove(key);
                                metadata_obj.remove(key);
                            } else {
                                if !current_obj.contains_key(key) {
                                    current_obj.insert(key.clone(), Value::Null);
                                }
                                if !metadata_obj.contains_key(key) {
                                    metadata_obj.insert(key.clone(), Value::Null);
                                }
                                let current_value = current_obj.get_mut(key).unwrap();
                                let metadata_entry = metadata_obj.get_mut(key).unwrap();
                                if !value.is_object() {
                                    *metadata_entry = Value::Number(timestamp.into());
                                }
                                update_recursive(current_value, value, metadata_entry, timestamp);
                            }
                        }
                    }
                }
                Value::Array(arr) => {
                    *current = Value::Array(arr.clone());
                    *metadata_value = Value::Number(timestamp.into());
                }
                Value::Null => {
                    *current = Value::Null;
                    *metadata_value = Value::Null;
                }
                _ => {
                    *current = update.clone();
                    *metadata_value = Value::Number(timestamp.into());
                }
            }
        }

        let timestamp = StateDocument::current_timestamp();
        // Update reported
        if update.reported.is_object() {
            update_recursive(
                &mut self.reported,
                &update.reported,
                &mut metadata.reported,
                timestamp,
            );
        }

        // Update desired
        if update.desired.is_object() {
            update_recursive(
                &mut self.desired,
                &update.desired,
                &mut metadata.desired,
                timestamp,
            );
        }
    }
}

#[cfg(test)]
mod tests;
