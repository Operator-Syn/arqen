pub(crate) fn redirect_spec(notice: Option<&str>, input: &str, _compact: bool) -> ModalSpec {
    let message = notice.unwrap_or("Paste the complete browser redirect URL:");
    ModalSpec {
        title: "Finish connection".into(),
        tone: ModalTone::Neutral,
        body: Text::from(vec![
            Line::from(message.to_owned()),
            Line::from(""),
            Line::from(format!("{input}|")),
        ]),
        actions: vec![
            action(
                ModalActionId::Continue,
                "SUBMIT",
                "Enter",
                ActionTone::Primary,
            ),
            action(ModalActionId::Cancel, "CANCEL", "Esc", ActionTone::Muted),
        ],
        focused_action: 0,
    }
}
