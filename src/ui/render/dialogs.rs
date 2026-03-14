use super::*;

pub(super) fn render_dialog(f: &mut Frame, area: Rect, app: &App) {
    let lang = app.language();
    let is_zh = matches!(lang, crate::i18n::Language::Chinese);

    if app.quit_confirm_dialog() {
        render_quit_confirm_dialog(f, area, is_zh);
        return;
    }

    if let Some(d) = app.new_session_dialog() {
        render_new_session_dialog(f, area, d, is_zh);
        return;
    }

    if let Some(d) = app.delete_confirm_dialog() {
        render_delete_confirm_dialog(f, area, d, is_zh);
        return;
    }

    if let Some(d) = app.delete_group_dialog() {
        render_delete_group_dialog(f, area, d, is_zh);
        return;
    }

    if let Some(d) = app.fork_dialog() {
        render_fork_dialog(f, area, d, is_zh);
        return;
    }

    if let Some(d) = app.create_group_dialog() {
        render_create_group_dialog(f, area, d, is_zh);
        return;
    }

    if let Some(d) = app.move_group_dialog() {
        render_move_group_dialog(f, area, d, is_zh);
        return;
    }

    if let Some(d) = app.rename_session_dialog() {
        render_rename_session_dialog(f, area, d, is_zh);
        return;
    }

    if let Some(d) = app.tag_picker_dialog() {
        render_tag_picker_dialog(f, area, d, is_zh);
        return;
    }

    if let Some(d) = app.rename_group_dialog() {
        render_rename_group_dialog(f, area, d, is_zh);
        return;
    }

    if let Some(d) = app.settings_dialog() {
        render_settings_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.control_request_dialog() {
        super::render_control_request_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.orphaned_rooms_dialog() {
        super::render_orphaned_rooms_dialog(f, area, d, is_zh);
        return;
    }

    if let Some(d) = app.pack_browser_dialog() {
        render_pack_browser_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.skills_manager_dialog() {
        super::render_skills_manager_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.join_session_dialog() {
        super::render_join_session_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.disconnect_viewer_dialog() {
        super::render_disconnect_viewer_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.share_dialog() {
        super::render_share_dialog(f, area, d, app);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.create_relationship_dialog() {
        super::render_create_relationship_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.annotate_dialog() {
        super::render_annotate_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.new_from_context_dialog() {
        super::render_new_from_context_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.human_review_dialog() {
        super::render_human_review_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.proposal_action_dialog() {
        super::render_proposal_action_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.confirm_injection_dialog() {
        super::render_confirm_injection_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "max")]
    if let Some(d) = app.ai_analysis_dialog() {
        super::render_ai_analysis_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "max")]
    if let Some(d) = app.behavior_analysis_dialog() {
        super::render_behavior_analysis_dialog(f, area, d, is_zh);
    }
}

pub(super) fn render_new_session_dialog(f: &mut Frame, area: Rect, d: &crate::ui::NewSessionDialog, is_zh: bool) {
    let popup_area = centered_rect(75, 60, area);
    f.render_widget(Clear, popup_area);

    let base_style = Style::default();
    let is_path_active = d.field == crate::ui::NewSessionField::Path;
    let is_title_active = d.field == crate::ui::NewSessionField::Title;
    let is_group_active = d.field == crate::ui::NewSessionField::Group;

    let mut path_spans = vec![Span::raw(if is_zh { "路径:   " } else { "Path:   " })];
    path_spans.extend(render_text_input(&d.path, is_path_active, base_style));

    let mut title_spans = vec![Span::raw(if is_zh { "标题:   " } else { "Title:  " })];
    title_spans.extend(render_text_input(&d.title, is_title_active, base_style));

    let mut group_spans = vec![Span::raw(if is_zh { "分组:   " } else { "Group:  " })];
    group_spans.extend(render_text_input(
        &d.group_path,
        is_group_active,
        base_style,
    ));

    let mut lines = vec![
        Line::from(Span::styled(
            if is_zh { "新建会话" } else { "New Session" },
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(path_spans),
    ];

    if d.path_will_be_created() {
        lines.push(Line::from(vec![
            Span::raw("        "),
            Span::styled(
                if is_zh { "(未找到; 将创建目录)" } else { "(not found; will create directory)" },
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    if d.path_suggestions_visible && !d.path_suggestions.is_empty() {
        lines.push(Line::from(vec![
            Span::raw("        "),
            Span::styled(if is_zh { "建议:" } else { "Suggestions:" }, Style::default().fg(Color::DarkGray)),
        ]));
        let max_show = 8usize;
        let len = d.path_suggestions.len();
        let idx = d.path_suggestions_idx.min(len.saturating_sub(1));
        let start = if len <= max_show {
            0
        } else if idx + 1 >= max_show {
            (idx + 1 - max_show).min(len - max_show)
        } else {
            0
        };

        for (i, s) in d
            .path_suggestions
            .iter()
            .enumerate()
            .skip(start)
            .take(max_show)
        {
            let style = if i == d.path_suggestions_idx {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            lines.push(Line::from(vec![
                Span::raw("          "),
                Span::styled(s.clone(), style),
            ]));
        }
    }

    lines.extend([Line::from(title_spans), Line::from(group_spans)]);

    if d.field == crate::ui::NewSessionField::Group {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            if is_zh { "分组 (↑/↓ 选择):" } else { "Groups (↑/↓ to select):" },
            Style::default().fg(Color::DarkGray),
        )));

        if d.group_matches.is_empty() {
            lines.push(Line::from(Span::styled(
                if is_zh { "(无匹配)" } else { "(no matches)" },
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            let max_show = 8usize;
            let len = d.group_matches.len();
            let idx = d.group_selected.min(len.saturating_sub(1));
            let start = if len <= max_show {
                0
            } else if idx + 1 >= max_show {
                (idx + 1 - max_show).min(len - max_show)
            } else {
                0
            };

            for (i, g) in d
                .group_matches
                .iter()
                .enumerate()
                .skip(start)
                .take(max_show)
            {
                let label = if g.is_empty() { if is_zh { "(无)" } else { "(none)" } } else { g.as_str() };
                let style = if i == d.group_selected {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(label.to_string(), style),
                ]));
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        if is_zh { "Tab: 补全路径 • ↑↓: 选择 • 回车: 下一个/提交 • Esc/Ctrl+C: 取消" } else { "Tab: complete path • ↑↓: pick • Enter: next/submit • Esc/Ctrl+C: cancel" },
        Style::default().fg(Color::DarkGray),
    )));

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "新建" } else { "New" }));

    f.render_widget(p, popup_area);
}

pub(super) fn render_fork_dialog(f: &mut Frame, area: Rect, d: &crate::ui::ForkDialog, is_zh: bool) {
    let popup_area = centered_rect(70, 40, area);
    f.render_widget(Clear, popup_area);

    let base_style = Style::default();
    let is_title_active = d.field == crate::ui::ForkField::Title;
    let is_group_active = d.field == crate::ui::ForkField::Group;

    let mut title_spans = vec![Span::raw(if is_zh { "标题: " } else { "Title: " })];
    title_spans.extend(render_text_input(&d.title, is_title_active, base_style));

    let mut group_spans = vec![Span::raw(if is_zh { "分组: " } else { "Group: " })];
    group_spans.extend(render_text_input(
        &d.group_path,
        is_group_active,
        base_style,
    ));

    let lines = vec![
        Line::from(Span::styled(
            if is_zh { "复制会话" } else { "Fork Session" },
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(title_spans),
        Line::from(group_spans),
        Line::from(""),
        Line::from(Span::styled(
            if is_zh { "Tab: 切换字段 • 回车: 下一个/提交 • Esc/Ctrl+C: 取消" } else { "Tab: switch field • Enter: next/submit • Esc/Ctrl+C: cancel" },
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "复制" } else { "Fork" }));

    f.render_widget(p, popup_area);
}

pub(super) fn render_create_group_dialog(f: &mut Frame, area: Rect, d: &crate::ui::CreateGroupDialog, is_zh: bool) {
    let popup_area = centered_rect(75, 60, area);
    f.render_widget(Clear, popup_area);

    let base_style = Style::default();
    let mut input_spans = vec![Span::raw(if is_zh { "名称:   " } else { "Name:   " })];
    input_spans.extend(render_text_input(&d.input, true, base_style));

    let mut lines = vec![
        Line::from(Span::styled(
            if is_zh { "创建分组" } else { "Create Group" },
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(input_spans),
        Line::from(""),
        Line::from(Span::styled(
            if is_zh { "已有分组 (↑/↓ 选择):" } else { "Existing (↑/↓ to select):" },
            Style::default().fg(Color::DarkGray),
        )),
    ];

    if d.matches.is_empty() {
        lines.push(Line::from(Span::styled(
            if is_zh { "(无匹配)" } else { "(no matches)" },
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let max_show = 10usize;
        let len = d.matches.len();
        let idx = d.selected.min(len.saturating_sub(1));
        let start = if len <= max_show {
            0
        } else if idx + 1 >= max_show {
            (idx + 1 - max_show).min(len - max_show)
        } else {
            0
        };

        for (i, g) in d.matches.iter().enumerate().skip(start).take(max_show) {
            let style = if i == d.selected {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(g.to_string(), style),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        if is_zh { "输入过滤/命名 • 回车: 创建 • Esc/Ctrl+C: 取消" } else { "Type to filter/name • Enter: create • Esc/Ctrl+C: cancel" },
        Style::default().fg(Color::DarkGray),
    )));

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "分组" } else { "Group" }));

    f.render_widget(p, popup_area);
}

pub(super) fn render_move_group_dialog(f: &mut Frame, area: Rect, d: &crate::ui::MoveGroupDialog, is_zh: bool) {
    let popup_area = centered_rect(75, 60, area);
    f.render_widget(Clear, popup_area);

    let base_style = Style::default();
    let mut input_spans = vec![Span::raw(if is_zh { "过滤: " } else { "Filter: " })];
    input_spans.extend(render_text_input(&d.input, true, base_style));

    let mut lines = vec![
        Line::from(Span::styled(
            if is_zh { "移动会话到分组" } else { "Move Session to Group" },
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw(if is_zh { "标题:  " } else { "Title:  " }),
            Span::styled(
                d.title.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(input_spans),
        Line::from(""),
        Line::from(Span::styled(
            if is_zh { "分组 (↑/↓ 选择):" } else { "Groups (↑/↓ to select):" },
            Style::default().fg(Color::DarkGray),
        )),
    ];

    if d.matches.is_empty() {
        lines.push(Line::from(Span::styled(
            if is_zh { "(无匹配)" } else { "(no matches)" },
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let max_show = 10usize;
        let len = d.matches.len();
        let idx = d.selected.min(len.saturating_sub(1));
        let start = if len <= max_show {
            0
        } else if idx + 1 >= max_show {
            (idx + 1 - max_show).min(len - max_show)
        } else {
            0
        };

        for (i, g) in d.matches.iter().enumerate().skip(start).take(max_show) {
            let label = if g.is_empty() { if is_zh { "(无)" } else { "(none)" } } else { g.as_str() };
            let style = if i == d.selected {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(label.to_string(), style),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        if is_zh { "输入过滤 • 回车: 应用 • Esc/Ctrl+C: 取消" } else { "Type to filter • Enter: apply • Esc/Ctrl+C: cancel" },
        Style::default().fg(Color::DarkGray),
    )));

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "分组" } else { "Group" }));

    f.render_widget(p, popup_area);
}

pub(super) fn render_tag_picker_dialog(f: &mut Frame, area: Rect, d: &crate::ui::TagPickerDialog, is_zh: bool) {
    let popup_area = centered_rect(60, 50, area);
    f.render_widget(Clear, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(popup_area);

    if d.tags.is_empty() {
        let empty = Paragraph::new(
            if is_zh { "(未找到标签)\n\n提示: 先编辑会话标签 (r), 然后在此处复用。" } else { "(no tags found)\n\nTip: edit a session label first (r), then reuse it here." },
        )
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "标签" } else { "Tag" }));
        f.render_widget(empty, chunks[0]);
    } else {
        let items: Vec<ListItem> = d
            .tags
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let fg = match t.color {
                    crate::session::LabelColor::Gray => Color::DarkGray,
                    crate::session::LabelColor::Magenta => Color::Magenta,
                    crate::session::LabelColor::Cyan => Color::Cyan,
                    crate::session::LabelColor::Green => Color::Green,
                    crate::session::LabelColor::Yellow => Color::Yellow,
                    crate::session::LabelColor::Red => Color::Red,
                    crate::session::LabelColor::Blue => Color::Blue,
                };

                let style = if i == d.selected {
                    Style::default()
                        .fg(fg)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(fg)
                };

                ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("[{}]", t.name), style),
                ]))
            })
            .collect();

        let list = List::new(items).block(Block::default().borders(Borders::ALL).title(if is_zh { "标签" } else { "Tag" }));
        let mut state = ListState::default().with_selected(Some(d.selected));
        f.render_stateful_widget(list, chunks[0], &mut state);
    }

    let hint = Paragraph::new(if is_zh { "↑/↓: 选择 • 回车: 应用 • Esc: 取消" } else { "↑/↓: select • Enter: apply • Esc: cancel" })
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(hint, chunks[1]);
}

pub(super) fn render_rename_session_dialog(f: &mut Frame, area: Rect, d: &crate::ui::RenameSessionDialog, is_zh: bool) {
    let popup_area = centered_rect(70, 45, area);
    f.render_widget(Clear, popup_area);

    let base_style = Style::default();
    let is_title_active = d.field == crate::ui::SessionEditField::Title;
    let is_label_active = d.field == crate::ui::SessionEditField::Label;
    let is_sid_active = d.field == crate::ui::SessionEditField::SessionId;

    let mut title_spans = vec![Span::raw(if is_zh { "标题:      " } else { "Title:      " })];
    title_spans.extend(render_text_input(&d.new_title, is_title_active, base_style));

    let mut label_spans = vec![Span::raw(if is_zh { "标签:      " } else { "Label:      " })];
    label_spans.extend(render_text_input(&d.label, is_label_active, base_style));

    let mut sid_spans = vec![Span::raw(if is_zh { "会话 ID:   " } else { "Session ID: " })];
    sid_spans.extend(render_text_input(&d.cli_session_id, is_sid_active, base_style));

    let (color_name, color_fg) = match d.label_color {
        crate::session::LabelColor::Gray => ("gray", Color::DarkGray),
        crate::session::LabelColor::Magenta => ("magenta", Color::Magenta),
        crate::session::LabelColor::Cyan => ("cyan", Color::Cyan),
        crate::session::LabelColor::Green => ("green", Color::Green),
        crate::session::LabelColor::Yellow => ("yellow", Color::Yellow),
        crate::session::LabelColor::Red => ("red", Color::Red),
        crate::session::LabelColor::Blue => ("blue", Color::Blue),
    };

    let color_style = if d.field == crate::ui::SessionEditField::Color {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let lines = vec![
        Line::from(Span::styled(
            if is_zh { "编辑会话" } else { "Edit Session" },
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(title_spans),
        Line::from(label_spans),
        Line::from(vec![
            Span::raw(if is_zh { "颜色:      " } else { "Color:      " }),
            Span::styled(
                format!("{color_name}"),
                color_style.fg(color_fg).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(sid_spans),
        Line::from(""),
        Line::from(vec![
            Span::styled("Tab", Style::default().fg(Color::Yellow)),
            Span::raw(if is_zh { ":下一字段  " } else { ":next field  " }),
            Span::styled("Enter", Style::default().fg(Color::Green)),
            Span::raw(if is_zh { ":下一个/应用  " } else { ":next/apply  " }),
            Span::styled("←/→", Style::default().fg(Color::Yellow)),
            Span::raw(if is_zh { ":颜色  " } else { ":color  " }),
            Span::styled("Esc", Style::default().fg(Color::DarkGray)),
            Span::raw(if is_zh { ":取消" } else { ":cancel" }),
        ]),
    ];

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "会话" } else { "Session" }));

    f.render_widget(p, popup_area);
}

pub(super) fn render_rename_group_dialog(f: &mut Frame, area: Rect, d: &crate::ui::RenameGroupDialog, is_zh: bool) {
    let popup_area = centered_rect(70, 35, area);
    f.render_widget(Clear, popup_area);

    let base_style = Style::default();
    let mut new_path_spans = vec![Span::raw(if is_zh { "到:    " } else { "To:    " })];
    new_path_spans.extend(render_text_input(&d.new_path, true, base_style));

    let lines = vec![
        Line::from(Span::styled(
            if is_zh { "重命名分组" } else { "Rename Group" },
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw(if is_zh { "从:    " } else { "From:  " }),
            Span::styled(d.old_path.clone(), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(new_path_spans),
        Line::from(""),
        Line::from(Span::styled(
            if is_zh { "回车: 应用 • Esc/Ctrl+C: 取消" } else { "Enter: apply • Esc/Ctrl+C: cancel" },
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "分组" } else { "Group" }));

    f.render_widget(p, popup_area);
}

pub(super) fn render_quit_confirm_dialog(f: &mut Frame, area: Rect, is_zh: bool) {
    let popup_area = centered_rect(40, 20, area);
    f.render_widget(Clear, popup_area);

    let lines = vec![
        Line::from(Span::styled(
            if is_zh { "退出 Agent Hand？" } else { "Quit Agent Hand?" },
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(if is_zh { "再按 q 退出。" } else { "Press q again to quit." }),
        Line::from(if is_zh { "按其他键取消。" } else { "Any other key to cancel." }),
    ];

    let p = Paragraph::new(lines)
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title(
            if is_zh { "确认退出" } else { "Confirm Quit" }
        ));

    f.render_widget(p, popup_area);
}

pub(super) fn render_delete_confirm_dialog(f: &mut Frame, area: Rect, d: &crate::ui::DeleteConfirmDialog, is_zh: bool) {
    let popup_area = centered_rect(60, 30, area);
    f.render_widget(Clear, popup_area);

    let lines = vec![
        Line::from(Span::styled(
            if is_zh { "删除会话？" } else { "Delete session?" },
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw(if is_zh { "标题: " } else { "Title: " }),
            Span::styled(
                d.title.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("ID:    "),
            Span::styled(d.session_id.clone(), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw(if is_zh { "终止 tmux 会话: " } else { "Kill tmux session: " }),
            Span::styled(
                if is_zh { if d.kill_tmux { "是" } else { "否" } } else { if d.kill_tmux { "YES" } else { "NO" } },
                if d.kill_tmux {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                },
            ),
            Span::raw(if is_zh { "  (按 't' 切换)" } else { "  (press 't' to toggle)" }),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            if is_zh { "y/回车: 确认 • n/Esc/Ctrl+C: 取消" } else { "y/Enter: confirm • n/Esc/Ctrl+C: cancel" },
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "确认" } else { "Confirm" }));

    f.render_widget(p, popup_area);
}

pub(super) fn render_delete_group_dialog(f: &mut Frame, area: Rect, d: &crate::ui::DeleteGroupDialog, is_zh: bool) {
    let popup_area = centered_rect(70, 35, area);
    f.render_widget(Clear, popup_area);

    let active = Style::default().fg(Color::Black).bg(Color::Cyan);

    let opt1_style = if d.choice == crate::ui::DeleteGroupChoice::DeleteGroupKeepSessions {
        active
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let opt2_style = if d.choice == crate::ui::DeleteGroupChoice::Cancel {
        active
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let opt3_style = if d.choice == crate::ui::DeleteGroupChoice::DeleteGroupAndSessions {
        active
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let lines = vec![
        Line::from(Span::styled(
            if is_zh { "删除分组？" } else { "Delete group?" },
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw(if is_zh { "分组: " } else { "Group: " }),
            Span::styled(
                d.group_path.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw(if is_zh { "会话数: " } else { "Sessions: " }),
            Span::styled(
                d.session_count.to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("1", Style::default().fg(Color::Yellow)),
            Span::raw(" "),
            Span::styled(if is_zh { "仅删除分组 (保留会话)" } else { "Delete group only (keep sessions)" }, opt1_style),
        ]),
        Line::from(vec![
            Span::styled("2", Style::default().fg(Color::Yellow)),
            Span::raw(" "),
            Span::styled(if is_zh { "取消" } else { "Cancel" }, opt2_style),
        ]),
        Line::from(vec![
            Span::styled("3", Style::default().fg(Color::Yellow)),
            Span::raw(" "),
            Span::styled(if is_zh { "删除分组和会话" } else { "Delete group + sessions" }, opt3_style),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            if is_zh { "1/2/3 或 ↑/↓ • 回车: 确认 • Esc/Ctrl+C: 取消" } else { "1/2/3 or ↑/↓ • Enter: confirm • Esc/Ctrl+C: cancel" },
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "确认" } else { "Confirm" }));

    f.render_widget(p, popup_area);
}

pub(super) fn render_search_popup(f: &mut Frame, area: Rect, app: &App) {
    let is_zh = matches!(app.language(), crate::i18n::Language::Chinese);
    let popup_area = centered_rect(80, 60, area);
    f.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        if is_zh { "搜索" } else { "Search" },
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::raw(format!(
        "{}: {}",
        if is_zh { "查询" } else { "Query" },
        app.search_query()
    ))));
    lines.push(Line::from(""));

    for (i, id) in app.search_results().iter().enumerate() {
        let s = app.session_by_id(id);
        let title = s.map(|x| x.title.as_str()).unwrap_or("<missing>");
        let group = s.map(|x| x.group_path.as_str()).unwrap_or("");
        let path = s
            .map(|x| x.project_path.to_string_lossy().to_string())
            .unwrap_or_default();

        let style = if i == app.search_selected() {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        lines.push(Line::from(vec![
            Span::styled(title.to_string(), style),
            Span::raw("  "),
            Span::styled(format!("[{}]", group), Style::default().fg(Color::Magenta)),
            Span::raw("  "),
            Span::styled(path, Style::default().fg(Color::DarkGray)),
        ]));
    }

    if app.search_results().is_empty() {
        lines.push(Line::from(Span::styled(
            "(no matches)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Type to filter • ↑/↓ to select • Enter to jump • Esc to close",
        Style::default().fg(Color::DarkGray),
    )));

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "搜索" } else { "Search" }));

    f.render_widget(p, popup_area);
}

pub(super) fn render_settings_dialog(
    f: &mut Frame,
    area: Rect,
    d: &crate::ui::SettingsDialog,
    is_zh: bool,
) {
    use crate::ui::{SettingsField, SettingsTab};

    let popup_area = centered_rect(70, 65, area);
    f.render_widget(Clear, popup_area);

    let base_style = Style::default();
    let active_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let dim_style = Style::default().fg(Color::DarkGray);

    let mut lines: Vec<Line<'static>> = Vec::new();

    // Title
    lines.push(Line::from(Span::styled(
        if is_zh { " 设置" } else { " Settings" },
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // Tab bar
    let tabs = SettingsTab::available_tabs();
    let mut tab_spans: Vec<Span<'static>> = Vec::new();
    tab_spans.push(Span::raw(" "));
    for (i, tab) in tabs.iter().enumerate() {
        if i > 0 {
            tab_spans.push(Span::styled("  ", dim_style));
        }
        let tab_label = if is_zh {
            match tab {
                SettingsTab::AI => "AI",
                SettingsTab::Sharing => "共享",
                SettingsTab::Notification => "音效",
                SettingsTab::General => "通用",
                SettingsTab::Keys => "快捷键",
            }
        } else {
            tab.label()
        };
        let label = format!(" {} ", tab_label);
        if *tab == d.tab {
            tab_spans.push(Span::styled(
                label,
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            tab_spans.push(Span::styled(label, dim_style));
        }
    }
    lines.push(Line::from(tab_spans));
    lines.push(Line::from(Span::styled(
        " ─".to_string() + &"─".repeat(popup_area.width.saturating_sub(4) as usize),
        dim_style,
    )));
    lines.push(Line::from(""));

    // Fields for current tab
    let fields = SettingsField::fields_for_tab(d.tab);
    for field in &fields {
        let is_active = *field == d.field;
        let field_label = if is_zh {
            match field {
                SettingsField::AiProvider => "AI 提供商",
                SettingsField::AiApiKey => "API 密钥",
                SettingsField::AiModel => "模型",
                SettingsField::AiBaseUrl => "Base URL",
                SettingsField::AiSummaryLines => "摘要行数",
                SettingsField::AiTest => "测试连接",
                SettingsField::RelayServerUrl => "中继服务器",
                SettingsField::TmateHost => "tmate 主机",
                SettingsField::TmatePort => "tmate 端口",
                SettingsField::DefaultPermission => "默认权限",
                SettingsField::AutoExpire => "自动过期 (分)",
                SettingsField::NotifHookStatus => "Hook 状态",
                SettingsField::NotifAutoRegister => "自动注册",
                SettingsField::NotifEnabled => "启用",
                SettingsField::NotifSoundPack => "音效包",
                SettingsField::NotifOnComplete => "完成时",
                SettingsField::NotifOnInput => "输入时",
                SettingsField::NotifOnError => "错误时",
                SettingsField::NotifVolume => "音量",
                SettingsField::NotifTestSound => "测试音效",
                SettingsField::NotifPackLink => "安装音效包",
                SettingsField::AnimationsEnabled => "动画",
                SettingsField::PromptCollection => "Prompt 收集",
                SettingsField::AnalyticsEnabled => "分析",
                SettingsField::MouseCapture => "鼠标捕获",
                SettingsField::JumpLines => "跳转行数",
                SettingsField::ScrollPadding => "滚动边距",
                SettingsField::ReadyTtl => "就绪 TTL (分)",
                SettingsField::Language => "语言",
                SettingsField::KeyUp => "上移",
                SettingsField::KeyDown => "下移",
                SettingsField::KeyHalfPageDown => "半页下",
                SettingsField::KeyHalfPageUp => "半页上",
                SettingsField::KeySelect => "选择",
                SettingsField::KeyStart => "启动会话",
                SettingsField::KeyStop => "停止会话",
                SettingsField::KeyRestart => "重启",
                SettingsField::KeyDelete => "删除",
                SettingsField::KeyRename => "重命名",
                SettingsField::KeyNewSession => "新建会话",
                SettingsField::KeyFork => "分叉",
                SettingsField::KeyCanvasToggle => "画布切换",
                SettingsField::KeySummarize => "AI 摘要",
                SettingsField::KeyBehaviorAnalysis => "行为分析",
                SettingsField::KeySearch => "搜索",
                SettingsField::KeySettings => "设置",
                SettingsField::KeyBoost => "加速",
            }
        } else {
            field.label()
        };
        let label = format!("  {:<16}", field_label);
        let label_style = if is_active { active_style } else { base_style };

        let mut spans: Vec<Span<'static>> = vec![Span::styled(label, label_style)];

        match field {
            SettingsField::AiProvider => {
                let is_editing_this = d.editing && is_active;
                if is_editing_this {
                    // Expanded chip selector: show all providers with wrapping
                    let max_width = popup_area.width.saturating_sub(22) as usize;
                    let mut row_width = 0usize;
                    let mut first_in_row = true;
                    let mut overflow_lines: Vec<Vec<Span<'static>>> = Vec::new();
                    let current_spans = &mut spans;

                    for (i, name) in d.ai_provider_names.iter().enumerate() {
                        let chip = format!(" {} ", name);
                        let chip_len = chip.len() + 1;
                        if !first_in_row && row_width + chip_len > max_width {
                            overflow_lines.push(Vec::new());
                            row_width = 0;
                            first_in_row = true;
                        }

                        let style = if i == d.ai_provider_idx {
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        };

                        let target = if overflow_lines.is_empty() {
                            &mut *current_spans
                        } else {
                            overflow_lines.last_mut().unwrap()
                        };
                        if !first_in_row {
                            target.push(Span::raw(" "));
                        }
                        target.push(Span::styled(chip, style));
                        row_width += chip_len;
                        first_in_row = false;
                    }

                    lines.push(Line::from(std::mem::take(current_spans)));
                    for row in overflow_lines {
                        let mut indented: Vec<Span<'static>> = vec![Span::raw(
                            " ".repeat(18),
                        )];
                        indented.extend(row);
                        lines.push(Line::from(indented));
                    }
                    continue;
                } else {
                    // Collapsed: show just the selected provider name
                    let val = format!("▸ {}", d.provider_display());
                    spans.push(Span::styled(val, if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            SettingsField::AiApiKey => {
                if d.editing && is_active {
                    spans.extend(render_text_input(&d.ai_api_key, true, base_style));
                } else {
                    let masked = d.masked_api_key();
                    let display = if masked.is_empty() {
                        "(not set)".to_string()
                    } else {
                        masked
                    };
                    spans.push(Span::styled(display, if is_active { active_style } else { dim_style }));
                }
            }
            SettingsField::AiModel => {
                if d.editing && is_active {
                    spans.extend(render_text_input(&d.ai_model, true, base_style));
                } else {
                    let t = d.ai_model.text();
                    let display = if t.is_empty() { "(provider default)" } else { t };
                    spans.push(Span::styled(display.to_string(), if is_active { active_style } else { dim_style }));
                }
            }
            SettingsField::AiBaseUrl => {
                if d.editing && is_active {
                    spans.extend(render_text_input(&d.ai_base_url, true, base_style));
                } else {
                    let t = d.ai_base_url.text();
                    let display = if t.is_empty() { "(provider default)" } else { t };
                    spans.push(Span::styled(display.to_string(), if is_active { active_style } else { dim_style }));
                }
            }
            SettingsField::AiSummaryLines => {
                if d.editing && is_active {
                    spans.extend(render_text_input(&d.ai_summary_lines, true, base_style));
                } else {
                    spans.push(Span::styled(
                        d.ai_summary_lines.text().to_string(),
                        if is_active { active_style } else { base_style },
                    ));
                }
            }
            SettingsField::AiTest => {
                if let Some(status) = &d.ai_test_status {
                    let color = if status.starts_with('✓') {
                        Color::Green
                    } else if status.starts_with('✗') {
                        Color::Red
                    } else {
                        Color::Yellow
                    };
                    spans.push(Span::styled(status.clone(), Style::default().fg(color)));
                } else {
                    spans.push(Span::styled(
                        "[press Enter to test]",
                        if is_active { active_style } else { dim_style },
                    ));
                }
            }
            SettingsField::DefaultPermission => {
                let is_editing_this = d.editing && is_active;
                let is_ro = d.default_permission != "rw";
                if is_editing_this {
                    let sel = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    let unsel = Style::default().fg(Color::DarkGray);
                    spans.push(Span::styled(" Read Only ", if is_ro { sel } else { unsel }));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(" Read/Write ", if !is_ro { sel } else { unsel }));
                } else {
                    let val = if is_ro { "Read Only" } else { "Read/Write" };
                    spans.push(Span::styled(format!("▸ {val}"), if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            SettingsField::PromptCollection => {
                let is_editing_this = d.editing && is_active;
                if is_editing_this {
                    let sel = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    let unsel = Style::default().fg(Color::DarkGray);
                    spans.push(Span::styled(" Off ", if !d.prompt_collection { sel } else { unsel }));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(" On ", if d.prompt_collection { sel } else { unsel }));
                } else {
                    let val = if d.prompt_collection { "On" } else { "Off" };
                    spans.push(Span::styled(format!("▸ {val}"), if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            SettingsField::AnimationsEnabled => {
                let is_editing_this = d.editing && is_active;
                if is_editing_this {
                    let sel = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    let unsel = Style::default().fg(Color::DarkGray);
                    spans.push(Span::styled(" Off ", if !d.animations_enabled { sel } else { unsel }));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(" On ", if d.animations_enabled { sel } else { unsel }));
                } else {
                    let val = if d.animations_enabled { "On" } else { "Off" };
                    spans.push(Span::styled(format!("▸ {val}"), if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            SettingsField::AnalyticsEnabled => {
                let is_editing_this = d.editing && is_active;
                if is_editing_this {
                    let sel = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    let unsel = Style::default().fg(Color::DarkGray);
                    spans.push(Span::styled(" Off ", if !d.analytics_enabled { sel } else { unsel }));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(" On ", if d.analytics_enabled { sel } else { unsel }));
                } else {
                    let val = if d.analytics_enabled { "On" } else { "Off" };
                    spans.push(Span::styled(format!("▸ {val}"), if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            SettingsField::MouseCapture => {
                let is_editing_this = d.editing && is_active;
                let labels = ["Auto", "On", "Off"];
                if is_editing_this {
                    let sel = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    let unsel = Style::default().fg(Color::DarkGray);
                    for (i, label) in labels.iter().enumerate() {
                        if i > 0 {
                            spans.push(Span::raw(" "));
                        }
                        let style = if d.mouse_capture_mode == i as u8 { sel } else { unsel };
                        spans.push(Span::styled(format!(" {label} "), style));
                    }
                } else {
                    let val = labels[d.mouse_capture_mode as usize % 3];
                    spans.push(Span::styled(format!("▸ {val}"), if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            // ── Notification tab fields — Hook Integration ──
            SettingsField::NotifHookStatus => {
                // Section header
                lines.push(Line::from(spans));
                // Show each tool's status
                for (i, info) in d.hook_tools.iter().enumerate() {
                    let is_sel = d.editing && is_active && i == d.hook_selected_tool;
                    let (sym, sym_color) = match info.status {
                        agent_hooks::ToolStatus::HooksRegistered => ("\u{2713}", Color::Green),
                        agent_hooks::ToolStatus::Detected => ("\u{25cf}", Color::Yellow),
                        agent_hooks::ToolStatus::NotInstalled => ("\u{2717}", Color::DarkGray),
                    };
                    let name_style = if is_sel {
                        Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
                    } else if info.status == agent_hooks::ToolStatus::NotInstalled {
                        dim_style
                    } else {
                        base_style
                    };
                    let mut row: Vec<Span<'static>> = vec![
                        Span::raw("    "),
                        Span::styled(format!("{sym} "), Style::default().fg(sym_color)),
                        Span::styled(format!("{:<16}", info.display_name), name_style),
                        Span::styled(info.status.label().to_string(), Style::default().fg(sym_color)),
                    ];
                    if is_sel && info.status == agent_hooks::ToolStatus::Detected {
                        row.push(Span::styled("  (Enter: install)", dim_style));
                    } else if is_sel && info.status == agent_hooks::ToolStatus::HooksRegistered {
                        row.push(Span::styled("  (Enter: uninstall)", dim_style));
                    }
                    lines.push(Line::from(row));
                }
                if !d.editing && is_active {
                    lines.push(Line::from(Span::styled(
                        "                  (Enter to manage)",
                        dim_style,
                    )));
                }
                continue;
            }
            SettingsField::NotifAutoRegister => {
                let is_editing_this = d.editing && is_active;
                if is_editing_this {
                    let sel = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    let unsel = Style::default().fg(Color::DarkGray);
                    spans.push(Span::styled(" Off ", if !d.hook_auto_register { sel } else { unsel }));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(" On ", if d.hook_auto_register { sel } else { unsel }));
                } else {
                    let val = if d.hook_auto_register { "On" } else { "Off" };
                    spans.push(Span::styled(format!("\u{25b8} {val}"), if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            // ── Notification tab fields — Sound section ──
            SettingsField::NotifEnabled => {
                let is_editing_this = d.editing && is_active;
                if is_editing_this {
                    let sel = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    let unsel = Style::default().fg(Color::DarkGray);
                    spans.push(Span::styled(" Off ", if !d.notif_enabled { sel } else { unsel }));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(" On ", if d.notif_enabled { sel } else { unsel }));
                } else {
                    let val = if d.notif_enabled { "On" } else { "Off" };
                    spans.push(Span::styled(format!("▸ {val}"), if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            SettingsField::NotifSoundPack => {
                let is_editing_this = d.editing && is_active;
                if is_editing_this {
                    // Expanded chip selector: show all pack names
                    let max_width = popup_area.width.saturating_sub(22) as usize;
                    let mut row_width = 0usize;
                    let mut first_in_row = true;
                    let mut overflow_lines: Vec<Vec<Span<'static>>> = Vec::new();
                    let current_spans = &mut spans;

                    if d.notif_pack_names.is_empty() {
                        current_spans.push(Span::styled(
                            "(no packs installed)",
                            dim_style,
                        ));
                    } else {
                        for (i, name) in d.notif_pack_names.iter().enumerate() {
                            let chip = format!(" {} ", name);
                            let chip_len = chip.len() + 1;
                            if !first_in_row && row_width + chip_len > max_width {
                                overflow_lines.push(Vec::new());
                                row_width = 0;
                                first_in_row = true;
                            }

                            let style = if i == d.notif_pack_idx {
                                Style::default()
                                    .fg(Color::Black)
                                    .bg(Color::Cyan)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(Color::DarkGray)
                            };

                            let target = if overflow_lines.is_empty() {
                                &mut *current_spans
                            } else {
                                overflow_lines.last_mut().unwrap()
                            };
                            if !first_in_row {
                                target.push(Span::raw(" "));
                            }
                            target.push(Span::styled(chip, style));
                            row_width += chip_len;
                            first_in_row = false;
                        }
                    }

                    lines.push(Line::from(std::mem::take(current_spans)));
                    for row in overflow_lines {
                        let mut indented: Vec<Span<'static>> = vec![Span::raw(
                            " ".repeat(18),
                        )];
                        indented.extend(row);
                        lines.push(Line::from(indented));
                    }
                    continue;
                } else {
                    let val = format!("▸ {}", d.pack_display());
                    spans.push(Span::styled(val, if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            SettingsField::NotifOnComplete => {
                let is_editing_this = d.editing && is_active;
                if is_editing_this {
                    let sel = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    let unsel = Style::default().fg(Color::DarkGray);
                    spans.push(Span::styled(" Off ", if !d.notif_on_complete { sel } else { unsel }));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(" On ", if d.notif_on_complete { sel } else { unsel }));
                } else {
                    let val = if d.notif_on_complete { "On" } else { "Off" };
                    spans.push(Span::styled(format!("▸ {val}"), if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            SettingsField::NotifOnInput => {
                let is_editing_this = d.editing && is_active;
                if is_editing_this {
                    let sel = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    let unsel = Style::default().fg(Color::DarkGray);
                    spans.push(Span::styled(" Off ", if !d.notif_on_input { sel } else { unsel }));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(" On ", if d.notif_on_input { sel } else { unsel }));
                } else {
                    let val = if d.notif_on_input { "On" } else { "Off" };
                    spans.push(Span::styled(format!("▸ {val}"), if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            SettingsField::NotifOnError => {
                let is_editing_this = d.editing && is_active;
                if is_editing_this {
                    let sel = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    let unsel = Style::default().fg(Color::DarkGray);
                    spans.push(Span::styled(" Off ", if !d.notif_on_error { sel } else { unsel }));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(" On ", if d.notif_on_error { sel } else { unsel }));
                } else {
                    let val = if d.notif_on_error { "On" } else { "Off" };
                    spans.push(Span::styled(format!("▸ {val}"), if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            SettingsField::NotifVolume => {
                if d.editing && is_active {
                    spans.extend(render_text_input(&d.notif_volume, true, base_style));
                    spans.push(Span::styled("%", base_style));
                } else {
                    let t = d.notif_volume.text();
                    spans.push(Span::styled(
                        format!("{t}%"),
                        if is_active { active_style } else { base_style },
                    ));
                }
            }
            SettingsField::NotifTestSound => {
                if let Some(status) = &d.notif_test_status {
                    let color = if status.starts_with('✓') {
                        Color::Green
                    } else if status.starts_with('✗') {
                        Color::Red
                    } else {
                        Color::Yellow
                    };
                    spans.push(Span::styled(status.clone(), Style::default().fg(color)));
                } else {
                    spans.push(Span::styled(
                        "[press Enter to test]",
                        if is_active { active_style } else { dim_style },
                    ));
                }
            }
            SettingsField::NotifPackLink => {
                if is_active {
                    spans.push(Span::styled(
                        "Enter to browse",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else {
                    spans.push(Span::styled(
                        "Browse & install",
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            }
            SettingsField::Language => {
                let is_editing_this = d.editing && is_active;
                let labels = ["English", "中文"];
                if is_editing_this {
                    let sel = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    let unsel = Style::default().fg(Color::DarkGray);
                    for (i, label) in labels.iter().enumerate() {
                        if i > 0 {
                            spans.push(Span::raw(" "));
                        }
                        let style = if d.language_idx == i { sel } else { unsel };
                        spans.push(Span::styled(format!(" {label} "), style));
                    }
                } else {
                    let val = labels[d.language_idx % 2];
                    spans.push(Span::styled(format!("▸ {val}"), if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            // Key binding fields
            f if f.is_key_binding() => {
                if d.key_capturing && is_active {
                    spans.push(Span::styled(
                        "⌨ Press a key...".to_string(),
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    ));
                } else {
                    let action = f.key_action().unwrap_or("");
                    let display = d.key_bindings.get(action)
                        .map(|specs| specs.iter().map(|k| crate::config::format_key_spec(k)).collect::<Vec<_>>().join(" / "))
                        .unwrap_or_else(|| "(not set)".to_string());
                    spans.push(Span::styled(
                        display,
                        if is_active { active_style } else { base_style },
                    ));
                    if is_active {
                        spans.push(Span::styled(
                            if is_zh { "  (回车:修改)" } else { "  (Enter:rebind)" },
                            dim_style,
                        ));
                    }
                }
            }
            // Text input fields: relay_url, auto_expire, jump_lines, scroll_padding, ready_ttl
            _ => {
                let input = match field {
                    SettingsField::RelayServerUrl => &d.relay_url,
                    SettingsField::AutoExpire => &d.auto_expire,
                    SettingsField::JumpLines => &d.jump_lines,
                    SettingsField::ScrollPadding => &d.scroll_padding,
                    SettingsField::ReadyTtl => &d.ready_ttl,
                    _ => unreachable!(),
                };
                if d.editing && is_active {
                    spans.extend(render_text_input(input, true, base_style));
                } else {
                    let t = input.text();
                    let display = if t.is_empty() { "(not set)" } else { t };
                    spans.push(Span::styled(
                        display.to_string(),
                        if is_active { active_style } else { base_style },
                    ));
                }
            }
        }

        lines.push(Line::from(spans));
    }

    // Spacing + status
    lines.push(Line::from(""));

    // Dirty indicator
    if d.dirty {
        lines.push(Line::from(Span::styled(
            if is_zh { "  ● 未保存的更改" } else { "  ● Unsaved changes" },
            Style::default().fg(Color::Yellow),
        )));
    }

    // Key hints
    lines.push(Line::from(""));
    let hint_style = Style::default().fg(Color::DarkGray);
    if d.key_capturing {
        lines.push(Line::from(Span::styled(
            if is_zh { "  按下新的快捷键...  Esc:取消" } else { "  Press new key...  Esc:cancel" },
            hint_style,
        )));
    } else if d.editing {
        let is_selector = d.field.is_selector();
        if is_selector {
            lines.push(Line::from(Span::styled(
                if is_zh { "  ←/→:选择  回车/Esc:完成" } else { "  ←/→:choose  Enter/Esc:done" },
                hint_style,
            )));
        } else {
            lines.push(Line::from(Span::styled(
                if is_zh { "  输入编辑  回车/Esc:完成" } else { "  type to edit  Enter/Esc:done" },
                hint_style,
            )));
        }
    } else {
        if is_zh {
            lines.push(Line::from(vec![
                Span::styled("  ←/→:切换标签  ", hint_style),
                Span::styled("j/k:选择字段  ", hint_style),
                Span::styled("回车:编辑  ", hint_style),
                Span::styled("Ctrl+S:保存  ", hint_style),
                Span::styled("Esc:关闭", hint_style),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("  ←/→:tab  ", hint_style),
                Span::styled("j/k:field  ", hint_style),
                Span::styled("Enter:edit  ", hint_style),
                Span::styled("Ctrl+S:save  ", hint_style),
                Span::styled("Esc:close", hint_style),
            ]));
        }
    }

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "设置" } else { "Settings" }));

    f.render_widget(p, popup_area);
}

pub(super) fn render_pack_browser_dialog(
    f: &mut Frame,
    area: Rect,
    d: &crate::ui::dialogs::PackBrowserDialog,
    is_zh: bool,
) {
    use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};

    let popup_area = centered_rect(60, 70, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(if is_zh { " 安装音效包 " } else { " Install Sound Packs " })
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Layout: status bar at top, pack list, hints at bottom
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // status
            Constraint::Min(4),   // pack list
            Constraint::Length(2), // hints
        ])
        .split(inner);

    // Status bar
    let status_style = if d.status.contains("Failed") || d.status.contains("failed") {
        Style::default().fg(Color::Red)
    } else if d.status.contains("Installed") || d.status.contains("Done") {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Yellow)
    };
    let status = Paragraph::new(format!(" {}", d.status)).style(status_style);
    f.render_widget(status, chunks[0]);

    // Pack list
    if d.loading {
        let loading = Paragraph::new(if is_zh { "  加载中..." } else { "  Loading..." })
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(loading, chunks[1]);
    } else if d.packs.is_empty() {
        let empty = Paragraph::new(if is_zh { "  未找到音效包" } else { "  No packs found" })
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(empty, chunks[1]);
    } else {
        let visible_height = chunks[1].height as usize;
        // Scroll to keep selected visible
        let scroll_offset = if d.selected >= visible_height {
            d.selected - visible_height + 1
        } else {
            0
        };

        let items: Vec<ListItem> = d
            .packs
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(visible_height)
            .map(|(i, pack)| {
                let marker = if pack.installed { if is_zh { " [已安装]" } else { " [installed]" } } else { "" };
                let text = format!("  {}{}", pack.name, marker);
                let style = if i == d.selected {
                    if pack.installed {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    }
                } else if pack.installed {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(text).style(style)
            })
            .collect();

        let list = List::new(items);
        f.render_widget(list, chunks[1]);
    }

    // Hints
    let hint = if is_zh {
        if d.installing {
            " 安装中... 请稍候".to_string()
        } else {
            " 回车: 安装  |  j/k: 浏览  |  Esc: 关闭\n 来源: github.com/PeonPing/og-packs".to_string()
        }
    } else {
        if d.installing {
            " Installing... please wait".to_string()
        } else {
            " Enter: Install  |  j/k: Navigate  |  Esc: Close\n Source: github.com/PeonPing/og-packs".to_string()
        }
    };
    let hints = Paragraph::new(hint)
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(hints, chunks[2]);
}









// Pro/Max render dialog functions are in pro/src/ui/render/dialogs*.rs
