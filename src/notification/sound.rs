//! Cross-platform sound playback.
//! macOS: afplay, Linux: pw-play / paplay / aplay

use std::path::Path;
use std::process::Stdio;

use tokio::process::Command;

/// Play a sound file asynchronously (non-blocking, fire-and-forget).
/// Volume is 0.0..=1.0.
pub fn play_async(path: &Path, volume: f32) {
    let path = path.to_path_buf();
    let volume = volume.clamp(0.0, 1.0);

    tokio::spawn(async move {
        if let Err(e) = play_platform(&path, volume).await {
            tracing::debug!("Sound playback failed: {}", e);
        }
    });
}

async fn play_platform(path: &Path, volume: f32) -> Result<(), String> {
    let path_str = path.to_string_lossy();

    if cfg!(target_os = "macos") {
        // afplay -v <volume> <file>
        Command::new("afplay")
            .args(["-v", &format!("{:.2}", volume), &path_str.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map_err(|e| format!("afplay: {}", e))?;
    } else {
        // Linux: try players in priority order
        let players: Vec<(&str, Vec<String>)> = vec![
            (
                "pw-play",
                vec!["--volume".into(), format!("{:.2}", volume), path_str.to_string()],
            ),
            (
                "paplay",
                vec![
                    format!("--volume={}", (volume * 65536.0) as u32),
                    path_str.to_string(),
                ],
            ),
            ("aplay", vec![path_str.to_string()]),
        ];

        for (cmd, args) in &players {
            let result = Command::new(cmd)
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;

            match result {
                Ok(status) if status.success() => return Ok(()),
                Ok(_) => continue,
                Err(_) => continue, // Command not found, try next
            }
        }

        return Err("No audio player found (tried pw-play, paplay, aplay)".into());
    }

    Ok(())
}
