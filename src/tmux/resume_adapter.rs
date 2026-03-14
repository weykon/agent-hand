use super::detector::Tool;

/// Describes a resume command built for a specific tool.
#[derive(Debug, Clone)]
pub struct ResumeSpec {
    pub command: String,
    pub supports_session_id: bool,
}

/// Build the resume command for a given tool + session ID.
///
/// `extra_flags` are additional CLI flags to preserve (e.g. `--model`, `--profile`).
pub fn build_resume_command(
    tool: Tool,
    session_id: &str,
    skip_perms: bool,
    extra_flags: &[String],
) -> Option<ResumeSpec> {
    let extras = if extra_flags.is_empty() {
        String::new()
    } else {
        format!(" {}", extra_flags.join(" "))
    };

    match tool {
        Tool::Claude => {
            let perms = if skip_perms {
                " --dangerously-skip-permissions"
            } else {
                ""
            };
            Some(ResumeSpec {
                command: format!("claude{}{} --resume {}", perms, extras, session_id),
                supports_session_id: true,
            })
        }
        Tool::Codex => {
            let perms = if skip_perms {
                " --full-auto"
            } else {
                ""
            };
            Some(ResumeSpec {
                command: format!("codex{} exec resume{} {}", perms, extras, session_id),
                supports_session_id: true,
            })
        }
        Tool::Gemini => {
            let perms = if skip_perms { " -y" } else { "" };
            Some(ResumeSpec {
                command: format!("gemini{}{} --resume {}", perms, extras, session_id),
                supports_session_id: true,
            })
        }
        _ => None,
    }
}

/// Parse session ID from a process's command-line args string.
/// Returns `(detected_tool, detected_session_id)`.
///
/// Unified parsing logic for all supported tools — the single source of truth
/// that matches `build_resume_command`.
pub fn parse_resume_args(args: &str) -> (Option<Tool>, Option<String>) {
    let parts: Vec<&str> = args.split_whitespace().collect();

    // Detect tool from binary name
    let tool = detect_tool_binary(&parts);

    // Parse session ID based on detected tool
    let session_id = match tool {
        Some(Tool::Claude) => parse_claude_session_id(&parts),
        Some(Tool::Codex) => parse_codex_session_id(&parts),
        Some(Tool::Gemini) => parse_gemini_session_id(&parts),
        _ => None,
    };

    (tool, session_id)
}

/// Detect tool from binary name in argument list.
fn detect_tool_binary(parts: &[&str]) -> Option<Tool> {
    for part in parts {
        let basename = part.rsplit('/').next().unwrap_or(part);
        match basename {
            "claude" => return Some(Tool::Claude),
            "codex" => return Some(Tool::Codex),
            "gemini" => return Some(Tool::Gemini),
            "opencode" => return Some(Tool::OpenCode),
            _ => {}
        }
    }
    None
}

/// Parse Claude session ID: --resume <id>, -r <id>, --continue <id>, -c <id>,
/// --resume=<id>, --continue=<id>
fn parse_claude_session_id(parts: &[&str]) -> Option<String> {
    for (i, part) in parts.iter().enumerate() {
        if (*part == "--resume" || *part == "-r" || *part == "--continue" || *part == "-c")
            && i + 1 < parts.len()
        {
            let candidate = parts[i + 1];
            if looks_like_session_id(candidate) {
                return Some(candidate.to_string());
            }
        }
        // --resume=<id> form
        if let Some(rest) = part
            .strip_prefix("--resume=")
            .or_else(|| part.strip_prefix("--continue="))
        {
            if looks_like_session_id(rest) {
                return Some(rest.to_string());
            }
        }
    }
    None
}

/// Parse Codex session ID: codex exec resume <id>
/// Also supports --resume <id> / -r <id> for backwards compat with old parsing.
fn parse_codex_session_id(parts: &[&str]) -> Option<String> {
    // Pattern: codex exec resume <session-id>
    for (i, _) in parts.iter().enumerate() {
        if i + 2 < parts.len() && parts[i] == "exec" && parts[i + 1] == "resume" {
            let candidate = parts[i + 2];
            if looks_like_session_id(candidate) {
                return Some(candidate.to_string());
            }
        }
    }
    // Fallback: --resume <id>
    for (i, part) in parts.iter().enumerate() {
        if (*part == "--resume" || *part == "-r") && i + 1 < parts.len() {
            let candidate = parts[i + 1];
            if looks_like_session_id(candidate) {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

/// Parse Gemini session ID: gemini --resume <id>, -r <id>
fn parse_gemini_session_id(parts: &[&str]) -> Option<String> {
    for (i, part) in parts.iter().enumerate() {
        if (*part == "--resume" || *part == "-r") && i + 1 < parts.len() {
            let candidate = parts[i + 1];
            if looks_like_session_id(candidate) {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

/// Heuristic: does this string look like a CLI session ID?
/// UUIDs, hex strings, base64-ish strings of reasonable length.
pub fn looks_like_session_id(s: &str) -> bool {
    if s.is_empty() || s.len() < 8 || s.len() > 128 {
        return false;
    }
    // Must not start with '-' (that would be another flag)
    if s.starts_with('-') {
        return false;
    }
    // Allow alphanumeric, hyphens, underscores (covers UUIDs, hex, base64url)
    s.chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

/// Extract known safe CLI parameters from an original command string.
/// Returns flags that should be preserved on resume (e.g. --model, --profile).
pub fn extract_preserve_flags(original_command: &str) -> Vec<String> {
    let parts: Vec<&str> = original_command.split_whitespace().collect();
    let mut flags = Vec::new();

    // Known safe parameters to preserve (flag + value pairs)
    // Covers Claude (--model, --profile), Codex (--model, --provider), Gemini (--model, --sandbox)
    let preserve_keys = ["--model", "--profile", "--provider", "--sandbox", "-m", "-p"];

    for (i, part) in parts.iter().enumerate() {
        // Handle --key value form
        if preserve_keys.contains(part) && i + 1 < parts.len() {
            let val = parts[i + 1];
            if !val.starts_with('-') {
                flags.push(format!("{} {}", part, val));
            }
        }
        // Handle --key=value form
        for key in &preserve_keys {
            if let Some(rest) = part.strip_prefix(&format!("{}=", key)) {
                if !rest.is_empty() {
                    flags.push(format!("{}={}", key, rest));
                }
            }
        }
    }

    flags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_trip_claude() {
        let sid = "550e8400-e29b-41d4-a716-446655440000";
        let spec = build_resume_command(Tool::Claude, sid, false, &[]).unwrap();
        let (tool, parsed_id) = parse_resume_args(&spec.command);
        assert_eq!(tool, Some(Tool::Claude));
        assert_eq!(parsed_id.as_deref(), Some(sid));
    }

    #[test]
    fn test_round_trip_claude_skip_perms() {
        let sid = "abc12345def";
        let spec = build_resume_command(Tool::Claude, sid, true, &[]).unwrap();
        assert!(spec.command.contains("--dangerously-skip-permissions"));
        let (tool, parsed_id) = parse_resume_args(&spec.command);
        assert_eq!(tool, Some(Tool::Claude));
        assert_eq!(parsed_id.as_deref(), Some(sid));
    }

    #[test]
    fn test_round_trip_codex() {
        let sid = "codex_session_abc123";
        let spec = build_resume_command(Tool::Codex, sid, false, &[]).unwrap();
        assert_eq!(spec.command, "codex exec resume codex_session_abc123");
        let (tool, parsed_id) = parse_resume_args(&spec.command);
        assert_eq!(tool, Some(Tool::Codex));
        assert_eq!(parsed_id.as_deref(), Some(sid));
    }

    #[test]
    fn test_round_trip_codex_skip_perms() {
        let sid = "codex_session_abc123";
        let spec = build_resume_command(Tool::Codex, sid, true, &[]).unwrap();
        assert!(spec.command.contains("--full-auto"));
        let (tool, parsed_id) = parse_resume_args(&spec.command);
        assert_eq!(tool, Some(Tool::Codex));
        assert_eq!(parsed_id.as_deref(), Some(sid));
    }

    #[test]
    fn test_round_trip_gemini() {
        let sid = "gemini_session_xyz789";
        let spec = build_resume_command(Tool::Gemini, sid, false, &[]).unwrap();
        assert_eq!(spec.command, "gemini --resume gemini_session_xyz789");
        let (tool, parsed_id) = parse_resume_args(&spec.command);
        assert_eq!(tool, Some(Tool::Gemini));
        assert_eq!(parsed_id.as_deref(), Some(sid));
    }

    #[test]
    fn test_round_trip_gemini_skip_perms() {
        let sid = "gemini_session_xyz789";
        let spec = build_resume_command(Tool::Gemini, sid, true, &[]).unwrap();
        assert!(spec.command.contains(" -y"));
        let (tool, parsed_id) = parse_resume_args(&spec.command);
        assert_eq!(tool, Some(Tool::Gemini));
        assert_eq!(parsed_id.as_deref(), Some(sid));
    }

    #[test]
    fn test_round_trip_with_extra_flags() {
        let sid = "abc12345def";
        let extras = vec!["--model gpt-4".to_string()];
        let spec = build_resume_command(Tool::Claude, sid, false, &extras).unwrap();
        assert!(spec.command.contains("--model gpt-4"));
        let (tool, parsed_id) = parse_resume_args(&spec.command);
        assert_eq!(tool, Some(Tool::Claude));
        assert_eq!(parsed_id.as_deref(), Some(sid));
    }

    #[test]
    fn test_shell_returns_none() {
        assert!(build_resume_command(Tool::Shell, "abc12345def", false, &[]).is_none());
    }

    #[test]
    fn test_parse_claude_variants() {
        let (t, id) = parse_resume_args("claude --resume abc12345def");
        assert_eq!(t, Some(Tool::Claude));
        assert_eq!(id.as_deref(), Some("abc12345def"));

        let (t, id) = parse_resume_args("claude -r abc12345def");
        assert_eq!(t, Some(Tool::Claude));
        assert_eq!(id.as_deref(), Some("abc12345def"));

        let (t, id) = parse_resume_args("claude --resume=abc12345def");
        assert_eq!(t, Some(Tool::Claude));
        assert_eq!(id.as_deref(), Some("abc12345def"));

        let (t, id) = parse_resume_args("claude --continue abc12345def");
        assert_eq!(t, Some(Tool::Claude));
        assert_eq!(id.as_deref(), Some("abc12345def"));
    }

    #[test]
    fn test_parse_codex_exec_resume() {
        let (t, id) = parse_resume_args("codex exec resume session123abc");
        assert_eq!(t, Some(Tool::Codex));
        assert_eq!(id.as_deref(), Some("session123abc"));
    }

    #[test]
    fn test_parse_gemini_resume() {
        let (t, id) = parse_resume_args("gemini --resume session_xyz");
        assert_eq!(t, Some(Tool::Gemini));
        assert_eq!(id.as_deref(), Some("session_xyz"));

        let (t, id) = parse_resume_args("gemini -r session_xyz");
        assert_eq!(t, Some(Tool::Gemini));
        assert_eq!(id.as_deref(), Some("session_xyz"));
    }

    #[test]
    fn test_no_session_id() {
        let (t, id) = parse_resume_args("claude chat");
        assert_eq!(t, Some(Tool::Claude));
        assert_eq!(id, None);
    }

    #[test]
    fn test_extract_preserve_flags() {
        let flags = extract_preserve_flags("claude --model gpt-4 --resume abc");
        assert_eq!(flags, vec!["--model gpt-4"]);

        let flags = extract_preserve_flags("claude --model=gpt-4 --profile dev");
        assert_eq!(flags, vec!["--model=gpt-4", "--profile dev"]);

        let flags = extract_preserve_flags("claude --resume abc");
        assert!(flags.is_empty());
    }

    #[test]
    fn test_looks_like_session_id() {
        assert!(looks_like_session_id("abc12345"));
        assert!(looks_like_session_id("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!looks_like_session_id(""));
        assert!(!looks_like_session_id("short"));
        assert!(!looks_like_session_id("--flag"));
    }
}
