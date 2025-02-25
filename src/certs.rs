use std::process::Command;
use std::path::Path;
use serde::{Serialize, Deserialize};
use thiserror::Error;
use std::io;

#[derive(Debug, Error)]
pub enum CfsslError {
    #[error("Failed to execute cfssl command: {0}")]
    CommandExecution(#[from] io::Error),
    
    #[error("cfssl command failed: {0}")]
    CommandFailed(String),
    
    #[error("Failed to parse cfssl output as UTF-8: {0}")]
    Utf8Parse(#[from] std::string::FromUtf8Error),
    
    #[error("Failed to parse cfssl JSON response: {0}")]
    JsonParse(#[from] serde_json::Error),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CfsslResponse {
    pub cert: String,
    pub key: String,
    pub csr: String,
}

pub fn generate_client_certificate(client_id: &str, cert_dir: &Path, cfssl_path: Option<&Path>, organization: Option<&str>) -> Result<CfsslResponse, CfsslError> {
    // Create the JSON request
    let mut json = serde_json::json!({
        "CN": client_id,
        "hosts": [""],
        "key": {
            "algo": "rsa",
            "size": 2048
        }
    });

    if let Some(org) = organization {
        json["names"] = serde_json::json!([{"O": org}]);
    }

    let request = json.to_string();

    // Set the current directory if provided
    let mut cfssl_command = "cfssl".to_string();
    if let Some(dir) = cfssl_path {
        // check if dir is a directory
        if dir.is_dir() {
            cfssl_command = dir.join("cfssl").to_string_lossy().to_string();
        } else {
            cfssl_command = dir.to_string_lossy().to_string();
        }
    }

    // generate ca.prem, ca-key.pem and cfssl.json paths
    let ca_cert_path = Path::new(cert_dir).join("ca.pem");
    let ca_key_path = Path::new(cert_dir).join("ca-key.pem");
    let cfssl_json_path = Path::new(cert_dir).join("cfssl.json");

    // check if the ca.pem, ca-key.pem and cfssl.json files exist
    if !ca_cert_path.exists(){
        return Err(CfsslError::CommandFailed(format!("No CA Cert: {} does not exist", ca_cert_path.to_string_lossy())));
    }
    if !ca_key_path.exists(){
        return Err(CfsslError::CommandFailed(format!("No CA Key: {} does not exist", ca_key_path.to_string_lossy())));
    }
    if !cfssl_json_path.exists(){
        return Err(CfsslError::CommandFailed(format!("No CFSSL Config: {} does not exist", cfssl_json_path.to_string_lossy())));
    }

    // format the shell command
    let shell_command = format!(
        "echo '{}' | {} gencert -ca {} -ca-key {} -config {} -profile client -",
        request,
        cfssl_command,
        ca_cert_path.to_string_lossy(),
        ca_key_path.to_string_lossy(),
        cfssl_json_path.to_string_lossy()
    );

    // Execute the shell command
    let output = Command::new("sh")
        .arg("-c")
        .arg(&shell_command)
        .output()?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(CfsslError::CommandFailed(error.to_string()));
    }

    // Parse the JSON output
    let output_str = String::from_utf8(output.stdout)?;
    let response: CfsslResponse = serde_json::from_str(&output_str)?;

    Ok(response)
}