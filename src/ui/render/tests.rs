//! Render tests — verify TUI output as strings using ratatui's TestBackend.
//!
//! These tests render dialogs and canvas views to an in-memory buffer,
//! then extract the rendered text for assertion. No real terminal needed.

#[cfg(test)]
mod render_tests {
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;
    use std::path::PathBuf;

    /// Extract all text from a TestBackend's buffer as a single string.
    /// Joins rows with newlines, trims trailing whitespace per row.
    fn buffer_to_string(terminal: &Terminal<TestBackend>) -> String {
        let buf = terminal.backend().buffer();
        let area = buf.area;
        let mut lines = Vec::new();
        for y in area.y..area.bottom() {
            let mut row = String::new();
            for x in area.x..area.right() {
                let cell = &buf[(x, y)];
                row.push_str(cell.symbol());
            }
            lines.push(row.trim_end().to_string());
        }
        lines.join("\n")
    }

    // ── ConfirmInjectionDialog ───────────────────────────────────

    #[cfg(feature = "pro")]
    fn make_confirm_dialog() -> crate::ui::ConfirmInjectionDialog {
        crate::ui::ConfirmInjectionDialog {
            proposal_id: "prop-001".to_string(),
            reason: "Blocker in auth service needs resolution".to_string(),
            urgency: "Medium".to_string(),
            targets: vec![
                crate::ui::InjectionTarget {
                    session_key: "session_alpha".to_string(),
                    project_path: PathBuf::from("/tmp/project-a"),
                    selected: true,
                },
                crate::ui::InjectionTarget {
                    session_key: "session_beta".to_string(),
                    project_path: PathBuf::from("/tmp/project-b"),
                    selected: false,
                },
                crate::ui::InjectionTarget {
                    session_key: "session_gamma".to_string(),
                    project_path: PathBuf::from("/tmp/project-c"),
                    selected: true,
                },
            ],
            cursor: 0,
        }
    }

    #[cfg(feature = "pro")]
    #[test]
    fn confirm_injection_dialog_shows_targets_with_checkboxes() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let dialog = make_confirm_dialog();
        let area = Rect::new(0, 0, 100, 30);

        terminal.draw(|f| {
            super::super::render_confirm_injection_dialog(f, area, &dialog, false);
        }).unwrap();

        let output = buffer_to_string(&terminal);

        // Title
        assert!(
            output.contains("Confirm Injection Targets"),
            "should show dialog title. Output:\n{}",
            output
        );

        // Reason + urgency
        assert!(
            output.contains("Blocker in auth service"),
            "should show reason text. Output:\n{}",
            output
        );
        assert!(
            output.contains("Medium"),
            "should show urgency level. Output:\n{}",
            output
        );

        // Session targets with checkboxes
        assert!(
            output.contains("[x] session_alpha"),
            "selected target should show [x]. Output:\n{}",
            output
        );
        assert!(
            output.contains("[ ] session_beta"),
            "unselected target should show [ ]. Output:\n{}",
            output
        );
        assert!(
            output.contains("[x] session_gamma"),
            "selected target should show [x]. Output:\n{}",
            output
        );

        // Key hints
        assert!(
            output.contains("Space: Toggle"),
            "should show key hints. Output:\n{}",
            output
        );
        assert!(
            output.contains("Enter: Inject"),
            "should show inject hint. Output:\n{}",
            output
        );
    }

    #[cfg(feature = "pro")]
    #[test]
    fn confirm_injection_dialog_chinese_mode() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let dialog = make_confirm_dialog();
        let area = Rect::new(0, 0, 100, 30);

        terminal.draw(|f| {
            super::super::render_confirm_injection_dialog(f, area, &dialog, true);
        }).unwrap();

        let output = buffer_to_string(&terminal);

        // ratatui TestBackend renders wide (CJK) characters with a space after each char,
        // so "确认注入目标" becomes "确 认 注 入 目 标" in the buffer output.
        assert!(
            output.contains("确") && output.contains("认") && output.contains("注") && output.contains("入") && output.contains("目") && output.contains("标"),
            "Chinese mode should show Chinese title characters. Output:\n{}",
            output
        );
        assert!(
            output.contains("全") && output.contains("选"),
            "Chinese mode should show Chinese hints. Output:\n{}",
            output
        );
    }

    #[cfg(feature = "pro")]
    #[test]
    fn confirm_injection_dialog_cursor_position() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut dialog = make_confirm_dialog();
        dialog.cursor = 1; // Cursor on session_beta
        let area = Rect::new(0, 0, 100, 30);

        terminal.draw(|f| {
            super::super::render_confirm_injection_dialog(f, area, &dialog, false);
        }).unwrap();

        let output = buffer_to_string(&terminal);

        // session_beta should still be visible (the cursor highlights it with color,
        // but the text content remains the same)
        assert!(
            output.contains("session_beta"),
            "cursor target should be visible. Output:\n{}",
            output
        );
    }

    #[cfg(feature = "pro")]
    #[test]
    fn confirm_injection_dialog_toggle_and_select_all() {
        let mut dialog = make_confirm_dialog();

        // Initial state: alpha=selected, beta=unselected, gamma=selected
        assert_eq!(dialog.selected_targets().len(), 2);

        // Toggle cursor (at 0 = alpha) → deselect
        dialog.toggle_current();
        assert_eq!(dialog.selected_targets().len(), 1);
        assert!(!dialog.targets[0].selected);

        // Select all
        dialog.select_all();
        assert_eq!(dialog.selected_targets().len(), 3);
        assert!(dialog.targets.iter().all(|t| t.selected));
    }

    // ── ProposalActionDialog ────────────────────────────────────

    #[cfg(feature = "pro")]
    #[test]
    fn proposal_action_dialog_shows_content() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let dialog = crate::ui::ProposalActionDialog {
            proposal_id: "prop-test".to_string(),
            reason: "API rate limit needs backoff strategy".to_string(),
            source_session_id: "sid-abc123def456".to_string(),
            urgency: "High".to_string(),
            targets: vec!["target-1".to_string(), "target-2".to_string()],
            current_status: "Pending".to_string(),
            created_at: std::time::Instant::now(),
        };
        let area = Rect::new(0, 0, 100, 30);

        terminal.draw(|f| {
            super::super::render_proposal_action_dialog(f, area, &dialog, false);
        }).unwrap();

        let output = buffer_to_string(&terminal);

        assert!(
            output.contains("Followup Proposal"),
            "should show dialog title. Output:\n{}",
            output
        );
        assert!(
            output.contains("API rate limit"),
            "should show reason. Output:\n{}",
            output
        );
        assert!(
            output.contains("Urgency: High"),
            "should show urgency. Output:\n{}",
            output
        );
        assert!(
            output.contains("Pending"),
            "should show status. Output:\n{}",
            output
        );
    }

    // ── Canvas render ───────────────────────────────────────────

    #[test]
    fn canvas_renders_empty_state() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let canvas = crate::ui::canvas::CanvasState::default();
        let area = Rect::new(0, 0, 80, 24);

        terminal.draw(|f| {
            crate::ui::canvas::render::render_canvas(f, area, &canvas, false, false);
        }).unwrap();

        let output = buffer_to_string(&terminal);

        assert!(
            output.contains("Canvas"),
            "should show canvas title. Output:\n{}",
            output
        );
    }

    #[test]
    fn canvas_renders_with_nodes() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut canvas = crate::ui::canvas::CanvasState::default();

        // Add a test node
        use crate::ui::canvas::{CanvasOp, NodeKind};
        canvas.apply_op(CanvasOp::AddNode {
            id: "test-node-1".to_string(),
            label: "Auth Service".to_string(),
            kind: NodeKind::Process,
            pos: Some((2, 2)),
            content: None,
        });

        let area = Rect::new(0, 0, 120, 40);

        terminal.draw(|f| {
            crate::ui::canvas::render::render_canvas(f, area, &canvas, true, false);
        }).unwrap();

        let output = buffer_to_string(&terminal);

        assert!(
            output.contains("Auth Service"),
            "should render node label. Output:\n{}",
            output
        );
    }

    #[test]
    fn canvas_focused_vs_unfocused_has_title() {
        // Unfocused
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let canvas = crate::ui::canvas::CanvasState::default();
        let area = Rect::new(0, 0, 80, 24);

        terminal.draw(|f| {
            crate::ui::canvas::render::render_canvas(f, area, &canvas, false, false);
        }).unwrap();
        let unfocused = buffer_to_string(&terminal);

        // Focused
        let backend2 = TestBackend::new(80, 24);
        let mut terminal2 = Terminal::new(backend2).unwrap();

        terminal2.draw(|f| {
            crate::ui::canvas::render::render_canvas(f, area, &canvas, true, false);
        }).unwrap();
        let focused = buffer_to_string(&terminal2);

        // Both should have Canvas title
        assert!(unfocused.contains("Canvas"), "unfocused should have title");
        assert!(focused.contains("Canvas"), "focused should have title");
    }
}
