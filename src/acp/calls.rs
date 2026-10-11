//! What one message said about a tool call, and the call those messages add
//! up to.
//!
//! The protocol lets every report after the first carry only what changed
//! (*"All fields except `toolCallId` are optional in updates. Only the fields
//! being changed need to be included."*), and a permission request may name a
//! call by its id alone. So a report is forwarded as sent — nothing filled in,
//! no `""` for a title the agent did not send — and the reader keeps one record
//! per call id in which a field present replaces the stored value and a field
//! absent or `null` leaves it alone. The policy matcher reads that record.

use super::session::ToolRequest;
use std::path::PathBuf;

/// One message's account of a tool call. Every field but the id is `None`
/// when the message did not carry it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolReport {
    pub id: String,
    pub title: Option<String>,
    /// The protocol's spelling: `execute`, `edit`, ...
    pub kind: Option<String>,
    /// `pending`, `in_progress`, `completed`, `failed`.
    pub status: Option<String>,
    pub raw_input: Option<serde_json::Value>,
    /// `Some(vec![])` is *the agent said none*; `None` is *not said*.
    pub locations: Option<Vec<PathBuf>>,
}

impl ToolReport {
    /// Whether it says anything besides the call's content.
    pub fn says_something(&self) -> bool {
        self.title.is_some()
            || self.kind.is_some()
            || self.status.is_some()
            || self.raw_input.is_some()
            || self.locations.is_some()
    }

    /// From the protocol's own JSON shape of a tool call or tool call update.
    /// Read from JSON so that an absent key and a `null` are the same `None`,
    /// whatever the schema's defaults make of either.
    pub fn from_wire(id: String, fields: &serde_json::Value) -> Self {
        let text = |k: &str| fields.get(k).and_then(|v| v.as_str()).map(str::to_string);
        Self {
            id,
            title: text("title"),
            kind: text("kind"),
            status: text("status"),
            raw_input: fields.get("rawInput").filter(|v| !v.is_null()).cloned(),
            locations: fields
                .get("locations")
                .and_then(|v| v.as_array())
                .map(|ls| {
                    ls.iter()
                        .filter_map(|l| l.get("path").and_then(|p| p.as_str()))
                        .map(PathBuf::from)
                        .collect()
                }),
        }
    }
}

impl ToolRequest {
    /// Folds a later report in: a field it carries replaces the stored one; a
    /// field it omits leaves the stored one alone. Merging the same report
    /// twice is merging it once.
    pub fn merge(&mut self, r: &ToolReport) {
        if let Some(t) = &r.title {
            self.title = Some(crate::core::text::clip(t, 200));
        }
        if let Some(k) = &r.kind {
            self.kind = Some(k.clone());
        }
        if let Some(i) = &r.raw_input {
            self.raw_input = Some(i.clone());
        }
        if let Some(l) = &r.locations {
            self.locations = l.clone();
        }
    }

    /// The title for a person: the agent's, or *untitled call <id>* when it
    /// never sent one — never an empty string.
    pub fn shown_title(&self) -> String {
        match self.title.as_deref() {
            Some(t) if !t.is_empty() => t.to_string(),
            _ => format!("untitled call {}", self.id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn wire(v: serde_json::Value) -> ToolReport {
        ToolReport::from_wire("c1".into(), &v)
    }

    fn id_only(id: &str) -> ToolReport {
        ToolReport {
            id: id.into(),
            ..ToolReport::default()
        }
    }

    trait FromReport {
        fn from_report(r: &ToolReport) -> Self;
    }

    impl FromReport for ToolRequest {
        fn from_report(r: &ToolReport) -> Self {
            let mut call = ToolRequest {
                id: r.id.clone(),
                ..ToolRequest::default()
            };
            call.merge(r);
            call
        }
    }

    #[test]
    fn a_present_field_replaces_and_an_absent_or_null_one_keeps() {
        let mut call = ToolRequest::from_report(&wire(json!({
            "title": "Run a command", "kind": "execute",
            "rawInput": {"command": "ls"}, "locations": [{"path": "/a"}]
        })));
        call.merge(&wire(json!({"rawInput": {"command": "rm -rf build"}})));
        assert_eq!(call.title.as_deref(), Some("Run a command"));
        assert_eq!(call.kind.as_deref(), Some("execute"));
        assert_eq!(call.raw_input, Some(json!({"command": "rm -rf build"})));
        assert_eq!(call.locations, vec![PathBuf::from("/a")]);

        call.merge(&wire(json!({
            "title": null, "kind": null, "rawInput": null, "locations": null
        })));
        assert_eq!(call.title.as_deref(), Some("Run a command"), "null keeps");
        assert_eq!(call.kind.as_deref(), Some("execute"));
        assert_eq!(call.raw_input, Some(json!({"command": "rm -rf build"})));
        assert_eq!(call.locations, vec![PathBuf::from("/a")]);

        call.merge(&wire(json!({"title": "rm -rf build", "kind": "delete"})));
        assert_eq!(call.title.as_deref(), Some("rm -rf build"), "later wins");
        assert_eq!(call.kind.as_deref(), Some("delete"));
    }

    #[test]
    fn an_empty_location_list_is_said_and_a_missing_one_is_not() {
        let mut call = ToolRequest::from_report(&wire(json!({"locations": [{"path": "/a"}]})));
        call.merge(&wire(json!({})));
        assert_eq!(call.locations.len(), 1, "None keeps");
        call.merge(&wire(json!({"locations": []})));
        assert!(call.locations.is_empty(), "Some(vec![]) replaces with none");
        assert_eq!(wire(json!({"locations": []})).locations, Some(vec![]));
        assert_eq!(wire(json!({})).locations, None);
    }

    #[test]
    fn merging_a_full_report_twice_is_merging_it_once() {
        let full = wire(json!({
            "title": "t", "kind": "execute", "status": "pending",
            "rawInput": {"command": "x"}, "locations": [{"path": "/p"}]
        }));
        let mut once = ToolRequest::from_report(&full);
        let twice = {
            let mut c = once.clone();
            c.merge(&full);
            c
        };
        once.merge(&id_only("c1"));
        assert_eq!(once, twice);
    }

    #[test]
    fn an_id_only_report_carries_nothing_and_says_nothing() {
        let r = wire(json!({}));
        assert_eq!(r, id_only("c1"));
        assert!(!r.says_something());
        let call = ToolRequest::from_report(&r);
        assert_eq!(call.title, None);
        assert_eq!(call.kind, None);
        assert_eq!(call.raw_input, None);
    }

    #[test]
    fn an_untitled_call_is_named_never_blank() {
        let call = ToolRequest::from_report(&id_only("toolu_9"));
        assert_eq!(call.shown_title(), "untitled call toolu_9");
        let blank = ToolRequest::from_report(&wire(json!({"title": ""})));
        assert_eq!(blank.shown_title(), "untitled call c1");
        assert_ne!(call.shown_title(), "");
    }
}
