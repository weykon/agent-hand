//! agent-hand-bridge — lightweight sync IPC bridge for agent-hand.
//!
//! Fast companion binary (~400K, ~2ms startup) for programmatic interaction
//! with a running agent-hand instance. Designed for AI agents and scripts.
//!
//! Modes:
//! - **Hook events** (default, no args): CLI tool hook → normalize → push via socket/JSONL
//! - **Canvas ops** (`canvas <json>`): Send CanvasOp to running TUI, get response
//! - **Canvas query** (`query <what>`): Shortcut for canvas query (nodes/edges/state)
//! - **Session** (`session <cmd>`): Session CRUD and lifecycle via control socket
//! - **Group** (`group <cmd>`): Group management via control socket
//! - **Rel** (`rel <cmd>`): Relationship management via control socket
//! - **Status** (`status`): Overall status report via control socket
//! - **Control** (`control <json>`): Raw JSON control op via control socket
//! - **Ping** (`ping`): Check if agent-hand is running and responsive
//!
//! Design constraints:
//! - **Pure sync** — no tokio runtime, ~2ms startup.
//! - **Never fail loudly** in hook mode — exit(0) on all errors.
//! - Canvas/query/session/group/rel/status/control modes DO report errors to stderr.

use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_hand::hooks::{HookEvent, HookEventKind, HookUsage};

/// Max prompt chars to keep (matches the old Python bridge).
const MAX_PROMPT_CHARS: usize = 2000;

/// Max chars to keep for tool_input and tool_response payloads.
const MAX_TOOL_CHARS: usize = 4000;

/// Truncate a string at a UTF-8 safe boundary.
fn truncate_utf8(s: &str, max_chars: usize) -> &str {
    if s.len() <= max_chars {
        return s;
    }
    // floor_char_boundary finds the largest byte index <= max_chars that is a char boundary.
    &s[..s.floor_char_boundary(max_chars)]
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        // `agent-hand-bridge canvas '{"op":"add_node",...}'`
        // `echo '{"op":"add_node",...}' | agent-hand-bridge canvas -`
        Some("canvas") => {
            let exit_code = run_canvas(&args[2..]);
            std::process::exit(exit_code);
        }

        // `agent-hand-bridge query nodes`
        // `agent-hand-bridge query state`
        Some("query") => {
            let what = args.get(2).map(|s| s.as_str()).unwrap_or("state");
            let kind = find_flag_value(&args[2..], "--kind");
            let label = find_flag_value(&args[2..], "--label");
            let id_filter = find_flag_value(&args[2..], "--id");

            let mut parts = vec![
                r#""op":"query""#.to_string(),
                format!(r#""what":"{}""#, what),
            ];
            if let Some(k) = kind {
                parts.push(format!(r#""kind":"{}""#, escape_json(&k)));
            }
            if let Some(l) = label {
                parts.push(format!(r#""label_contains":"{}""#, escape_json(&l)));
            }
            if let Some(i) = id_filter {
                parts.push(format!(r#""id":"{}""#, escape_json(&i)));
            }

            let op = format!("{{{}}}", parts.join(","));
            let exit_code = canvas_roundtrip(&op);
            std::process::exit(exit_code);
        }

        // `agent-hand-bridge session <subcmd> [args...]`
        Some("session") => {
            let exit_code = run_session_cmd(&args[2..]);
            std::process::exit(exit_code);
        }

        // `agent-hand-bridge group <subcmd> [args...]`
        Some("group") => {
            let exit_code = run_group_cmd(&args[2..]);
            std::process::exit(exit_code);
        }

        // `agent-hand-bridge rel <subcmd> [args...]`
        Some("rel") => {
            let exit_code = run_rel_cmd(&args[2..]);
            std::process::exit(exit_code);
        }

        // `agent-hand-bridge status [--json]`
        Some("status") => {
            let exit_code = run_status(&args[2..]);
            std::process::exit(exit_code);
        }

        // `agent-hand-bridge control '{"op":"list_sessions"}'`
        Some("control") => {
            let json = args.get(2).map(|s| s.as_str()).unwrap_or("{}");
            let exit_code = control_roundtrip(json);
            std::process::exit(exit_code);
        }

        // `agent-hand-bridge ping`
        Some("ping") => {
            let exit_code = run_ping();
            std::process::exit(exit_code);
        }

        // `agent-hand-bridge --help` / `agent-hand-bridge help`
        Some("--help" | "-h" | "help") => {
            print_usage();
            std::process::exit(0);
        }

        // Default: hook event mode (read stdin JSON, normalize, deliver)
        _ => {
            if let Err(_) = run_hook() {
                // Silently exit — never interfere with the host CLI tool's hook pipeline.
            }
        }
    }
}

fn print_usage() {
    eprintln!(
        r#"agent-hand-bridge — lightweight IPC bridge for agent-hand

USAGE:
  agent-hand-bridge                    Hook mode: read event JSON from stdin, push to agent-hand
  agent-hand-bridge canvas '<json>'    Send a canvas operation (JSON), print response
  agent-hand-bridge canvas -           Read canvas operation JSON from stdin
  agent-hand-bridge canvas --batch <file>  Read array of ops from file, send as Batch
  agent-hand-bridge query <what> [--kind K] [--label L] [--id I]
                                       Query canvas: nodes | edges | state | selected
  agent-hand-bridge ping               Check if agent-hand is running

SESSION COMMANDS:
  agent-hand-bridge session list [--group G] [--tag T] [--status S]
  agent-hand-bridge session add <path> [--title T] [--group G] [--cmd C]
  agent-hand-bridge session remove <id>
  agent-hand-bridge session info <id>
  agent-hand-bridge session start <id>
  agent-hand-bridge session stop <id>
  agent-hand-bridge session restart <id>
  agent-hand-bridge session resume <id>
  agent-hand-bridge session interrupt <id>
  agent-hand-bridge session send <id> <text...>
  agent-hand-bridge session rename <id> <new-title>
  agent-hand-bridge session label <id> <label> [--color C]
  agent-hand-bridge session move <id> <group>
  agent-hand-bridge session tag <id> <tag>
  agent-hand-bridge session untag <id> <tag>
  agent-hand-bridge session pane <id> [--lines N]   Read last N lines of tmux output
  agent-hand-bridge session progress <id>            Read session progress file

GROUP COMMANDS:
  agent-hand-bridge group list
  agent-hand-bridge group create <path>
  agent-hand-bridge group delete <path>
  agent-hand-bridge group rename <old> <new>

RELATIONSHIP COMMANDS:
  agent-hand-bridge rel add <a-id> <b-id> [--type T] [--label L]
  agent-hand-bridge rel remove <id>
  agent-hand-bridge rel list [--session S]

OTHER:
  agent-hand-bridge status [--json]    Overall status report
  agent-hand-bridge control '<json>'   Raw control JSON operation

SOCKET PATHS:
  Hook events:  ~/.agent-hand/events/hook.sock
  Canvas ops:   ~/.agent-hand/canvas.sock
  Control ops:  ~/.agent-hand/control.sock"#
    );
}

// ── Canvas mode ──────────────────────────────────────────────────────────

fn run_canvas(args: &[String]) -> i32 {
    let json = match args.first().map(|s| s.as_str()) {
        // Read from stdin
        Some("-") | None => {
            let mut buf = String::new();
            if io::stdin().read_to_string(&mut buf).is_err() {
                eprintln!("error: failed to read stdin");
                return 1;
            }
            buf
        }
        // Batch mode: read file, wrap as Batch op
        Some("--batch") => {
            let file = match args.get(1) {
                Some(f) => f,
                None => {
                    eprintln!("error: --batch requires a file path");
                    return 1;
                }
            };
            let content = match std::fs::read_to_string(file) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("error: cannot read {file}: {e}");
                    return 1;
                }
            };
            // Wrap array of ops in a Batch op
            format!(r#"{{"op":"batch","ops":{}}}"#, content.trim())
        }
        // Inline JSON argument
        Some(json) => json.to_string(),
    };

    canvas_roundtrip(&json)
}

/// Send a single canvas op JSON to canvas.sock, print the response. Returns exit code.
fn canvas_roundtrip(json: &str) -> i32 {
    let socket_path = agent_hand_dir().join("canvas.sock");
    socket_roundtrip(&socket_path, json, "canvas")
}

/// Send a single control op JSON to control.sock, print the response. Returns exit code.
fn control_roundtrip(json: &str) -> i32 {
    let socket_path = agent_hand_dir().join("control.sock");
    socket_roundtrip(&socket_path, json, "control")
}

/// Generic socket roundtrip: send JSON, read response line, print it.
#[cfg(not(unix))]
fn socket_roundtrip(_socket_path: &PathBuf, _json: &str, label: &str) -> i32 {
    eprintln!("error: {label} socket is only available on Unix platforms");
    1
}

/// Generic socket roundtrip: send JSON, read response line, print it.
#[cfg(unix)]
fn socket_roundtrip(socket_path: &PathBuf, json: &str, label: &str) -> i32 {
    let mut stream = match UnixStream::connect(socket_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "error: cannot connect to {label} socket at {}: {e}",
                socket_path.display()
            );
            eprintln!("hint: is agent-hand running?");
            return 1;
        }
    };

    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(2)));
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));

    // Send JSON line
    let mut payload = json.trim().to_string();
    payload.push('\n');
    if let Err(e) = stream.write_all(payload.as_bytes()) {
        eprintln!("error: failed to send to {label} socket: {e}");
        return 1;
    }

    // Shut down write half to signal we're done sending
    let _ = stream.shutdown(std::net::Shutdown::Write);

    // Read response line
    let reader = io::BufReader::new(&stream);
    for line in reader.lines() {
        match line {
            Ok(line) if !line.trim().is_empty() => {
                println!("{}", line);
                return 0;
            }
            Ok(_) => continue,
            Err(e) => {
                eprintln!("error: failed to read {label} response: {e}");
                return 1;
            }
        }
    }

    eprintln!("error: no response from {label} socket");
    1
}

// ── Session commands ─────────────────────────────────────────────────────

fn run_session_cmd(args: &[String]) -> i32 {
    let subcmd = match args.first().map(|s| s.as_str()) {
        Some(s) => s,
        None => {
            eprintln!("error: session subcommand required (list|add|remove|info|start|stop|restart|resume|interrupt|send|rename|label|move|tag|untag)");
            return 1;
        }
    };

    let json = match subcmd {
        "list" => {
            let group = find_flag_value(args, "--group");
            let tag = find_flag_value(args, "--tag");
            let status = find_flag_value(args, "--status");
            let mut parts = vec![r#""op":"list_sessions""#.to_string()];
            if let Some(g) = group {
                parts.push(format!(r#""group":"{}""#, escape_json(&g)));
            }
            if let Some(t) = tag {
                parts.push(format!(r#""tag":"{}""#, escape_json(&t)));
            }
            if let Some(s) = status {
                parts.push(format!(r#""status":"{}""#, escape_json(&s)));
            }
            format!("{{{}}}", parts.join(","))
        }
        "add" => {
            let path = match args.get(1) {
                Some(p) => p.clone(),
                None => {
                    eprintln!("error: session add requires <path>");
                    return 1;
                }
            };
            let title = find_flag_value(args, "--title");
            let group = find_flag_value(args, "--group");
            let command = find_flag_value(args, "--cmd");
            let mut parts = vec![
                r#""op":"add_session""#.to_string(),
                format!(r#""path":"{}""#, escape_json(&path)),
            ];
            if let Some(t) = title {
                parts.push(format!(r#""title":"{}""#, escape_json(&t)));
            }
            if let Some(g) = group {
                parts.push(format!(r#""group":"{}""#, escape_json(&g)));
            }
            if let Some(c) = command {
                parts.push(format!(r#""command":"{}""#, escape_json(&c)));
            }
            format!("{{{}}}", parts.join(","))
        }
        "remove" => {
            let id = match args.get(1) {
                Some(i) => i,
                None => {
                    eprintln!("error: session remove requires <id>");
                    return 1;
                }
            };
            format!(r#"{{"op":"remove_session","id":"{}"}}"#, escape_json(id))
        }
        "info" => {
            let id = match args.get(1) {
                Some(i) => i,
                None => {
                    eprintln!("error: session info requires <id>");
                    return 1;
                }
            };
            format!(r#"{{"op":"session_info","id":"{}"}}"#, escape_json(id))
        }
        "start" => {
            let id = match args.get(1) {
                Some(i) => i,
                None => {
                    eprintln!("error: session start requires <id>");
                    return 1;
                }
            };
            format!(r#"{{"op":"start_session","id":"{}"}}"#, escape_json(id))
        }
        "stop" => {
            let id = match args.get(1) {
                Some(i) => i,
                None => {
                    eprintln!("error: session stop requires <id>");
                    return 1;
                }
            };
            format!(r#"{{"op":"stop_session","id":"{}"}}"#, escape_json(id))
        }
        "restart" => {
            let id = match args.get(1) {
                Some(i) => i,
                None => {
                    eprintln!("error: session restart requires <id>");
                    return 1;
                }
            };
            format!(r#"{{"op":"restart_session","id":"{}"}}"#, escape_json(id))
        }
        "rename" => {
            let id = match args.get(1) {
                Some(i) => i,
                None => {
                    eprintln!("error: session rename requires <id> <new-title>");
                    return 1;
                }
            };
            let title = match args.get(2) {
                Some(t) => t,
                None => {
                    eprintln!("error: session rename requires <id> <new-title>");
                    return 1;
                }
            };
            format!(
                r#"{{"op":"rename_session","id":"{}","title":"{}"}}"#,
                escape_json(id),
                escape_json(title)
            )
        }
        "label" => {
            let id = match args.get(1) {
                Some(i) => i,
                None => {
                    eprintln!("error: session label requires <id> <label>");
                    return 1;
                }
            };
            let label = match args.get(2) {
                Some(l) => l,
                None => {
                    eprintln!("error: session label requires <id> <label>");
                    return 1;
                }
            };
            let color = find_flag_value(args, "--color");
            let mut parts = vec![
                r#""op":"set_label""#.to_string(),
                format!(r#""id":"{}""#, escape_json(id)),
                format!(r#""label":"{}""#, escape_json(label)),
            ];
            if let Some(c) = color {
                parts.push(format!(r#""color":"{}""#, escape_json(&c)));
            }
            format!("{{{}}}", parts.join(","))
        }
        "move" => {
            let id = match args.get(1) {
                Some(i) => i,
                None => {
                    eprintln!("error: session move requires <id> <group>");
                    return 1;
                }
            };
            let group = match args.get(2) {
                Some(g) => g,
                None => {
                    eprintln!("error: session move requires <id> <group>");
                    return 1;
                }
            };
            format!(
                r#"{{"op":"move_session","id":"{}","group":"{}"}}"#,
                escape_json(id),
                escape_json(group)
            )
        }
        "tag" => {
            let id = match args.get(1) {
                Some(i) => i,
                None => {
                    eprintln!("error: session tag requires <id> <tag>");
                    return 1;
                }
            };
            let tag = match args.get(2) {
                Some(t) => t,
                None => {
                    eprintln!("error: session tag requires <id> <tag>");
                    return 1;
                }
            };
            format!(
                r#"{{"op":"add_tag","id":"{}","tag":"{}"}}"#,
                escape_json(id),
                escape_json(tag)
            )
        }
        "untag" => {
            let id = match args.get(1) {
                Some(i) => i,
                None => {
                    eprintln!("error: session untag requires <id> <tag>");
                    return 1;
                }
            };
            let tag = match args.get(2) {
                Some(t) => t,
                None => {
                    eprintln!("error: session untag requires <id> <tag>");
                    return 1;
                }
            };
            format!(
                r#"{{"op":"remove_tag","id":"{}","tag":"{}"}}"#,
                escape_json(id),
                escape_json(tag)
            )
        }
        "pane" => {
            let id = match args.get(1) {
                Some(i) => i,
                None => {
                    eprintln!("error: session pane requires <id>");
                    return 1;
                }
            };
            let lines = find_flag_value(args, "--lines").unwrap_or_else(|| "30".to_string());
            format!(
                r#"{{"op":"read_pane","id":"{}","lines":{}}}"#,
                escape_json(id),
                lines
            )
        }
        "progress" => {
            let id = match args.get(1) {
                Some(i) => i,
                None => {
                    eprintln!("error: session progress requires <id>");
                    return 1;
                }
            };
            format!(r#"{{"op":"read_progress","id":"{}"}}"#, escape_json(id))
        }
        "resume" => {
            let id = match args.get(1) {
                Some(i) => i,
                None => {
                    eprintln!("error: session resume requires <id>");
                    return 1;
                }
            };
            format!(r#"{{"op":"resume_session","id":"{}"}}"#, escape_json(id))
        }
        "interrupt" => {
            let id = match args.get(1) {
                Some(i) => i,
                None => {
                    eprintln!("error: session interrupt requires <id>");
                    return 1;
                }
            };
            format!(r#"{{"op":"interrupt_session","id":"{}"}}"#, escape_json(id))
        }
        "send" => {
            let id = match args.get(1) {
                Some(i) => i,
                None => {
                    eprintln!("error: session send requires <id> <text...>");
                    return 1;
                }
            };
            if args.len() < 3 {
                eprintln!("error: session send requires <id> <text...>");
                return 1;
            }
            let text: String = args[2..].join(" ");
            format!(
                r#"{{"op":"send_prompt","id":"{}","text":"{}"}}"#,
                escape_json(id),
                escape_json(&text)
            )
        }
        other => {
            eprintln!("error: unknown session subcommand: {other}");
            return 1;
        }
    };

    control_roundtrip(&json)
}

// ── Group commands ───────────────────────────────────────────────────────

fn run_group_cmd(args: &[String]) -> i32 {
    let subcmd = match args.first().map(|s| s.as_str()) {
        Some(s) => s,
        None => {
            eprintln!("error: group subcommand required (list|create|delete|rename)");
            return 1;
        }
    };

    let json = match subcmd {
        "list" => r#"{"op":"list_groups"}"#.to_string(),
        "create" => {
            let path = match args.get(1) {
                Some(p) => p,
                None => {
                    eprintln!("error: group create requires <path>");
                    return 1;
                }
            };
            format!(r#"{{"op":"create_group","path":"{}"}}"#, escape_json(path))
        }
        "delete" => {
            let path = match args.get(1) {
                Some(p) => p,
                None => {
                    eprintln!("error: group delete requires <path>");
                    return 1;
                }
            };
            format!(r#"{{"op":"delete_group","path":"{}"}}"#, escape_json(path))
        }
        "rename" => {
            let old = match args.get(1) {
                Some(o) => o,
                None => {
                    eprintln!("error: group rename requires <old> <new>");
                    return 1;
                }
            };
            let new_path = match args.get(2) {
                Some(n) => n,
                None => {
                    eprintln!("error: group rename requires <old> <new>");
                    return 1;
                }
            };
            format!(
                r#"{{"op":"rename_group","old_path":"{}","new_path":"{}"}}"#,
                escape_json(old),
                escape_json(new_path)
            )
        }
        other => {
            eprintln!("error: unknown group subcommand: {other}");
            return 1;
        }
    };

    control_roundtrip(&json)
}

// ── Relationship commands ────────────────────────────────────────────────

fn run_rel_cmd(args: &[String]) -> i32 {
    let subcmd = match args.first().map(|s| s.as_str()) {
        Some(s) => s,
        None => {
            eprintln!("error: rel subcommand required (add|remove|list)");
            return 1;
        }
    };

    let json = match subcmd {
        "add" => {
            let a_id = match args.get(1) {
                Some(a) => a,
                None => {
                    eprintln!("error: rel add requires <a-id> <b-id>");
                    return 1;
                }
            };
            let b_id = match args.get(2) {
                Some(b) => b,
                None => {
                    eprintln!("error: rel add requires <a-id> <b-id>");
                    return 1;
                }
            };
            let rtype = find_flag_value(args, "--type");
            let label = find_flag_value(args, "--label");
            let mut parts = vec![
                r#""op":"add_relationship""#.to_string(),
                format!(r#""session_a":"{}""#, escape_json(a_id)),
                format!(r#""session_b":"{}""#, escape_json(b_id)),
            ];
            if let Some(t) = rtype {
                parts.push(format!(r#""relation_type":"{}""#, escape_json(&t)));
            }
            if let Some(l) = label {
                parts.push(format!(r#""label":"{}""#, escape_json(&l)));
            }
            format!("{{{}}}", parts.join(","))
        }
        "remove" => {
            let id = match args.get(1) {
                Some(i) => i,
                None => {
                    eprintln!("error: rel remove requires <id>");
                    return 1;
                }
            };
            format!(
                r#"{{"op":"remove_relationship","id":"{}"}}"#,
                escape_json(id)
            )
        }
        "list" => {
            let session = find_flag_value(args, "--session");
            if let Some(s) = session {
                format!(
                    r#"{{"op":"list_relationships","session":"{}"}}"#,
                    escape_json(&s)
                )
            } else {
                r#"{"op":"list_relationships"}"#.to_string()
            }
        }
        other => {
            eprintln!("error: unknown rel subcommand: {other}");
            return 1;
        }
    };

    control_roundtrip(&json)
}

// ── Status command ───────────────────────────────────────────────────────

fn run_status(_args: &[String]) -> i32 {
    control_roundtrip(r#"{"op":"status"}"#)
}

// ── Ping mode ────────────────────────────────────────────────────────────

#[cfg(not(unix))]
fn run_ping() -> i32 {
    println!("agent-hand: socket ping is only available on Unix platforms");
    1
}

#[cfg(unix)]
fn run_ping() -> i32 {
    // Check hook socket
    let hook_alive = UnixStream::connect(events_dir().join("hook.sock")).is_ok();
    // Check canvas socket
    let canvas_alive = UnixStream::connect(agent_hand_dir().join("canvas.sock")).is_ok();
    // Check control socket
    let control_alive = UnixStream::connect(agent_hand_dir().join("control.sock")).is_ok();

    if hook_alive || canvas_alive || control_alive {
        println!(
            "agent-hand: running (hook={}, canvas={}, control={})",
            if hook_alive { "ok" } else { "down" },
            if canvas_alive { "ok" } else { "down" },
            if control_alive { "ok" } else { "down" },
        );
        0
    } else {
        println!("agent-hand: not running");
        1
    }
}

// ── Hook event mode (default) ────────────────────────────────────────────

fn run_hook() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Read stdin JSON
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let data: serde_json::Value = serde_json::from_str(input.trim())?;

    // 2. Detect tmux session name
    let tmux_session = detect_tmux_session();
    if tmux_session.is_empty() {
        // Not inside a tmux session managed by agent-hand — nothing to do.
        return Ok(());
    }

    // 3. Extract fields from the raw hook JSON
    let raw_event = data
        .get("hook_event_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let session_id = data
        .get("session_id")
        .or_else(|| data.get("conversation_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let cwd = data
        .get("cwd")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(tmux_pane_current_path);
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    // 4. Normalise event kind via event_map
    let kind = match normalise_event(raw_event, &data) {
        Some(k) => k,
        None => return Ok(()), // Unknown event — skip silently
    };

    // 5. Extract prompt text for UserPromptSubmit events
    let prompt = if matches!(kind, HookEventKind::UserPromptSubmit) {
        extract_prompt(&data)
    } else {
        None
    };

    // 6. Build HookEvent
    let event = HookEvent {
        tmux_session,
        kind,
        session_id,
        cwd,
        ts,
        prompt,
        usage: extract_usage(&data),
    };

    // 7. Serialize to JSON line
    let mut json_line = serde_json::to_string(&event)?;
    json_line.push('\n');

    // 8. Try socket delivery, fall back to JSONL file
    let socket_path = events_dir().join("hook.sock");
    if send_via_socket(&socket_path, json_line.as_bytes()) {
        return Ok(());
    }

    // Fallback: append to JSONL file
    let jsonl_path = events_dir().join("hook-events.jsonl");
    append_to_file(&jsonl_path, json_line.as_bytes());

    Ok(())
}

// ── Shared helpers ───────────────────────────────────────────────────────

/// Detect the tmux session name from the environment.
fn detect_tmux_session() -> String {
    if std::env::var("TMUX").unwrap_or_default().is_empty() {
        return String::new();
    }

    std::process::Command::new("tmux")
        .args(["display-message", "-p", "#{session_name}"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8(out.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// Fallback: get the current pane's working directory from tmux.
/// Used when the CLI tool doesn't send `cwd` in the hook event payload.
fn tmux_pane_current_path() -> String {
    std::process::Command::new("tmux")
        .args(["display-message", "-p", "#{pane_current_path}"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8(out.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// Map raw CLI event names to our normalised HookEventKind.
fn normalise_event(raw: &str, data: &serde_json::Value) -> Option<HookEventKind> {
    let mut map: HashMap<&str, HookEventKind> = HashMap::new();

    // Claude Code (PascalCase)
    map.insert("Stop", HookEventKind::Stop);
    map.insert("UserPromptSubmit", HookEventKind::UserPromptSubmit);
    map.insert(
        "Notification",
        HookEventKind::Notification {
            notification_type: data
                .get("notification_type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        },
    );
    map.insert(
        "PermissionRequest",
        HookEventKind::PermissionRequest {
            tool_name: data
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        },
    );
    map.insert(
        "PostToolUseFailure",
        HookEventKind::ToolFailure {
            tool_name: data
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            error: data
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        },
    );
    map.insert("SubagentStart", HookEventKind::SubagentStart);
    map.insert("PreCompact", HookEventKind::PreCompact);

    // Claude Code tool use events
    map.insert(
        "PreToolUse",
        HookEventKind::PreToolUse {
            tool_name: data
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            tool_input: {
                let input = data.get("tool_input").cloned().unwrap_or(serde_json::Value::Null);
                // Truncate serialized input for bounded storage
                let serialized = serde_json::to_string(&input).unwrap_or_default();
                if serialized.len() > MAX_TOOL_CHARS {
                    let truncated = truncate_utf8(&serialized, MAX_TOOL_CHARS);
                    serde_json::Value::String(truncated.to_string())
                } else {
                    input
                }
            },
            tool_use_id: data
                .get("tool_use_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        },
    );
    map.insert(
        "PostToolUse",
        HookEventKind::PostToolUse {
            tool_name: data
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            tool_input: {
                let input = data.get("tool_input").cloned().unwrap_or(serde_json::Value::Null);
                let serialized = serde_json::to_string(&input).unwrap_or_default();
                if serialized.len() > MAX_TOOL_CHARS {
                    let truncated = truncate_utf8(&serialized, MAX_TOOL_CHARS);
                    serde_json::Value::String(truncated.to_string())
                } else {
                    input
                }
            },
            tool_response: {
                let resp = data
                    .get("tool_response")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                truncate_utf8(resp, MAX_TOOL_CHARS).to_string()
            },
            tool_use_id: data
                .get("tool_use_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        },
    );

    // Cursor compatibility — map to proper tool use types
    map.insert("stop", HookEventKind::Stop);
    map.insert(
        "preToolUse",
        HookEventKind::PreToolUse {
            tool_name: data
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            tool_input: data.get("tool_input").cloned().unwrap_or(serde_json::Value::Null),
            tool_use_id: data
                .get("tool_use_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        },
    );
    map.insert(
        "postToolUse",
        HookEventKind::PostToolUse {
            tool_name: data
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            tool_input: data.get("tool_input").cloned().unwrap_or(serde_json::Value::Null),
            tool_response: data
                .get("tool_response")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            tool_use_id: data
                .get("tool_use_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        },
    );
    map.insert("subagentStop", HookEventKind::Stop);
    map.insert("subagentStart", HookEventKind::SubagentStart);
    map.insert("beforeSubmitPrompt", HookEventKind::UserPromptSubmit);
    map.insert("beforeShellExecution", HookEventKind::UserPromptSubmit);

    // Codex CLI
    map.insert("userPromptSubmitted", HookEventKind::UserPromptSubmit);
    map.insert(
        "errorOccurred",
        HookEventKind::ToolFailure {
            tool_name: data
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            error: data
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        },
    );

    // Windsurf
    map.insert("post_cascade_response", HookEventKind::Stop);
    map.insert("pre_user_prompt", HookEventKind::UserPromptSubmit);

    // Kiro
    map.insert("agentSpawn", HookEventKind::SubagentStart);
    map.insert("userPromptSubmit", HookEventKind::UserPromptSubmit);

    // Gemini CLI
    map.insert("turn_complete", HookEventKind::Stop);
    map.insert("user_prompt_submit", HookEventKind::UserPromptSubmit);

    map.remove(raw)
}

/// Extract user prompt text from various CLI payload formats.
fn extract_prompt(data: &serde_json::Value) -> Option<String> {
    let text = data
        .get("prompt")
        .and_then(|v| v.as_str())
        .or_else(|| data.get("user_prompt").and_then(|v| v.as_str()))
        .or_else(|| {
            data.get("input")
                .and_then(|v| v.get("prompt"))
                .and_then(|v| v.as_str())
        })
        .or_else(|| {
            data.get("tool_input")
                .and_then(|v| v.get("prompt"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or("");

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    let truncated = truncate_utf8(trimmed, MAX_PROMPT_CHARS);

    Some(truncated.to_string())
}

/// Extract structured token usage from common hook payload layouts.
fn extract_usage(data: &serde_json::Value) -> Option<HookUsage> {
    let candidates = [
        Some(data),
        data.get("usage"),
        data.get("token_usage"),
        data.get("metrics"),
        data.get("result").and_then(|v| v.get("usage")),
        data.get("result").and_then(|v| v.get("token_usage")),
        data.get("message").and_then(|v| v.get("usage")),
    ];

    let mut usage = HookUsage::default();
    for candidate in candidates.into_iter().flatten() {
        usage.input_tokens = usage
            .input_tokens
            .or_else(|| extract_u64(candidate, &["input_tokens", "prompt_tokens", "inputTokens", "promptTokens"]));
        usage.output_tokens = usage
            .output_tokens
            .or_else(|| extract_u64(candidate, &["output_tokens", "completion_tokens", "outputTokens", "completionTokens"]));
        usage.total_tokens = usage
            .total_tokens
            .or_else(|| extract_u64(candidate, &["total_tokens", "tokens", "totalTokens"]));
        usage.cache_creation_tokens = usage
            .cache_creation_tokens
            .or_else(|| extract_u64(candidate, &["cache_creation_tokens", "cacheCreationTokens"]));
        usage.cache_read_tokens = usage
            .cache_read_tokens
            .or_else(|| extract_u64(candidate, &["cache_read_tokens", "cacheReadTokens"]));
    }

    if usage.total_tokens.is_none() {
        usage.total_tokens = match (usage.input_tokens, usage.output_tokens) {
            (Some(input), Some(output)) => Some(input.saturating_add(output)),
            _ => None,
        };
    }

    if usage.input_tokens.is_none()
        && usage.output_tokens.is_none()
        && usage.total_tokens.is_none()
        && usage.cache_creation_tokens.is_none()
        && usage.cache_read_tokens.is_none()
    {
        None
    } else {
        Some(usage)
    }
}

fn extract_u64(data: &serde_json::Value, keys: &[&str]) -> Option<u64> {
    for key in keys {
        let Some(value) = data.get(*key) else {
            continue;
        };
        if let Some(n) = value.as_u64() {
            return Some(n);
        }
        if let Some(s) = value.as_str() {
            if let Ok(n) = s.parse::<u64>() {
                return Some(n);
            }
        }
    }
    None
}

/// Try to send data via Unix domain socket (fire-and-forget). Returns true on success.
#[cfg(unix)]
fn send_via_socket(path: &PathBuf, data: &[u8]) -> bool {
    let mut stream = match UnixStream::connect(path) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let _ = stream.set_write_timeout(Some(std::time::Duration::from_millis(500)));
    stream.write_all(data).is_ok()
}

/// Stub for non-Unix platforms — socket IPC not available.
#[cfg(not(unix))]
fn send_via_socket(_path: &PathBuf, _data: &[u8]) -> bool {
    false
}

/// Append data to a JSONL file (fallback path).
fn append_to_file(path: &PathBuf, data: &[u8]) {
    use std::fs::OpenOptions;
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(data);
    }
}

/// Find a `--flag value` pair in the args slice.
fn find_flag_value(args: &[String], flag: &str) -> Option<String> {
    for (i, arg) in args.iter().enumerate() {
        if arg == flag {
            return args.get(i + 1).cloned();
        }
    }
    None
}

/// Escape a string for JSON embedding (handles quotes and backslashes).
fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// `~/.agent-hand/events/`
fn events_dir() -> PathBuf {
    agent_hand_dir().join("events")
}

/// `~/.agent-hand/`
fn agent_hand_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".agent-hand")
}
