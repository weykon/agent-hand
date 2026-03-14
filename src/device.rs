use sha2::{Digest, Sha256};
use std::process::Command;

/// Information about the current device, used for device-slot enforcement.
pub struct DeviceInfo {
    /// SHA-256 hex digest (64 chars) — the unique device identifier.
    pub device_id: String,
    /// Human-readable hostname for display purposes.
    pub hostname: String,
    /// OS and architecture string, e.g. "macos-aarch64".
    pub os_arch: String,
}

impl DeviceInfo {
    /// Generate device info from local hardware signals.
    /// The raw signals are never transmitted — only the SHA-256 digest.
    pub fn generate() -> Self {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_default();

        let os_arch = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);

        let machine_id = get_machine_id();

        let mac = mac_address::get_mac_address()
            .ok()
            .flatten()
            .map(|m| m.to_string())
            .unwrap_or_default();

        let raw = format!("{hostname}|{os_arch}|{machine_id}|{mac}");
        let hash = Sha256::digest(raw.as_bytes());
        let device_id = hex::encode(hash);

        Self {
            device_id,
            hostname,
            os_arch,
        }
    }
}

/// Retrieve a platform-specific machine identifier.
fn get_machine_id() -> String {
    #[cfg(target_os = "macos")]
    {
        // IOPlatformUUID from IOPlatformExpertDevice
        if let Ok(output) = Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("IOPlatformUUID") {
                    // Format: "IOPlatformUUID" = "XXXXXXXX-..."
                    if let Some(uuid) = line.split('"').nth(3) {
                        return uuid.to_string();
                    }
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(id) = std::fs::read_to_string("/etc/machine-id") {
            let trimmed = id.trim().to_string();
            if !trimmed.is_empty() {
                return trimmed;
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = Command::new("wmic")
            .args(["csproduct", "get", "UUID"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Second line contains the UUID
            if let Some(uuid) = stdout.lines().nth(1) {
                let trimmed = uuid.trim().to_string();
                if !trimmed.is_empty() {
                    return trimmed;
                }
            }
        }
    }

    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_valid_sha256_hex() {
        let info = DeviceInfo::generate();
        assert_eq!(info.device_id.len(), 64, "device_id should be 64 hex chars");
        assert!(
            info.device_id.chars().all(|c| c.is_ascii_hexdigit()),
            "device_id should be valid hex"
        );
    }

    #[test]
    fn fingerprint_is_deterministic() {
        let a = DeviceInfo::generate();
        let b = DeviceInfo::generate();
        assert_eq!(a.device_id, b.device_id, "same machine should produce same fingerprint");
    }

    #[test]
    fn fields_are_populated() {
        let info = DeviceInfo::generate();
        assert!(!info.hostname.is_empty(), "hostname should not be empty");
        assert!(!info.os_arch.is_empty(), "os_arch should not be empty");
        assert!(info.os_arch.contains('-'), "os_arch should be OS-ARCH format");
    }

    #[test]
    fn machine_id_resolves() {
        let id = get_machine_id();
        // On CI or unusual systems this may be empty, but on macOS/Linux it should work
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        assert!(!id.is_empty(), "machine_id should resolve on macOS/Linux");
        let _ = id; // suppress unused warning on other platforms
    }
}
