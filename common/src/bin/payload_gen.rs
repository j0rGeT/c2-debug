use clap::Parser;
use c2_common::payload::{PayloadConfig, TargetOs, Architecture, generate_payload};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Server URL for the client to connect to
    #[arg(short, long, default_value = "127.0.0.1:8080")]
    server: String,
    
    /// Target operating system
    #[arg(short, long, value_enum, default_value = "linux")]
    os: OsTarget,
    
    /// Target architecture
    #[arg(short, long, value_enum, default_value = "x86_64")]
    arch: ArchTarget,
    
    /// Output path for the generated payload
    #[arg(short, long, default_value = "./payload")]
    output: PathBuf,
    
    /// Strip debug symbols from the binary
    #[arg(short, long, default_value = "true")]
    strip: bool,
    
    /// Compress with UPX
    #[arg(long, default_value = "false")]
    upx: bool,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum OsTarget {
    Windows,
    Linux,
    MacOS,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum ArchTarget {
    X86_64,
    Aarch64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    let config = PayloadConfig {
        server_url: args.server,
        target_os: match args.os {
            OsTarget::Windows => TargetOs::Windows,
            OsTarget::Linux => TargetOs::Linux,
            OsTarget::MacOS => TargetOs::MacOS,
        },
        architecture: match args.arch {
            ArchTarget::X86_64 => Architecture::X86_64,
            ArchTarget::Aarch64 => Architecture::Aarch64,
        },
        output_path: args.output.to_string_lossy().to_string(),
        strip: args.strip,
        upx: args.upx,
    };
    
    println!("Generating payload for {} {}", 
        match config.target_os {
            TargetOs::Windows => "Windows",
            TargetOs::Linux => "Linux", 
            TargetOs::MacOS => "macOS",
        },
        match config.architecture {
            Architecture::X86_64 => "x86_64",
            Architecture::Aarch64 => "aarch64",
        }
    );
    
    generate_payload(&config)?;
    
    println!("Payload generated successfully: {}", config.output_path);
    
    Ok(())
}