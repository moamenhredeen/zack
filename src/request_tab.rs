//! An open request tab: the draft the user is editing, and the view state that
//! belongs to it.
//!
//! The document in [`CollectionStore`](crate::collection_store::CollectionStore)
//! holds only what is on disk. Edits live here, in the input states, until a
//! save materialises them back into the document — so a tab *is* the draft, and
//! `is_dirty` is simply whether it has diverged from the saved request.
//!
//! Keeping the draft in the tab rather than in the document is what makes
//! reverting a single edit and resolving an external change possible: both need
//! the saved version to still be somewhere.

use gpui::{App, AppContext, Context, Entity, EventEmitter, SharedString, Window};
use gpui_component::{
    IndexPath,
    input::{InputEvent, InputState},
    select::{SelectEvent, SelectState},
};
use opencollection::{
    HttpBodyOrVariants, HttpRequestBody, HttpRequestDetails, HttpRequestHeader, HttpRequestParam,
    ParamType,
};

use crate::{
    collection_store::ItemPath,
    model::{METHODS, ResponseRecord},
};

/// Which request a tab is editing.
///
/// Positional, because Zack only ever appends requests: an `ItemPath` stays
/// valid for the life of a tab. Deleting or reordering requests would break
/// that, and this is the single place that would have to change — most likely
/// to `Item::source()`, the relative file path `save_item` already matches on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabTarget {
    pub collection: usize,
    pub item: ItemPath,
}

/// Which editor pane a tab is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestPane {
    Headers,
    Params,
    Body,
}

impl RequestPane {
    pub fn index(self) -> usize {
        match self {
            Self::Headers => 0,
            Self::Params => 1,
            Self::Body => 2,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Params,
            2 => Self::Body,
            _ => Self::Headers,
        }
    }
}

/// Which response pane a tab is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponsePane {
    Body,
    Headers,
    Meta,
}

impl ResponsePane {
    pub fn index(self) -> usize {
        match self {
            Self::Body => 0,
            Self::Headers => 1,
            Self::Meta => 2,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Headers,
            2 => Self::Meta,
            _ => Self::Body,
        }
    }
}

/// Emitted when the draft diverges from the saved request, so the tab bar can
/// redraw its unsaved marker.
pub struct TabDirtied;

pub struct RequestTab {
    pub target: TabTarget,
    pub title: String,
    /// `true` once an edit has landed that a save has not yet written.
    ///
    /// Set on any change event rather than by comparing against `saved`: typing
    /// a character and deleting it again leaves the tab marked dirty. Bruno
    /// behaves the same way, and the alternative is a structural comparison on
    /// every keystroke.
    pub is_dirty: bool,
    pub pane: RequestPane,
    pub response_pane: ResponsePane,
    pub response_body_pretty: bool,
    pub response: Option<ResponseRecord>,
    pub is_sending: bool,
    pub method: String,
    pub method_select: Entity<SelectState<Vec<SharedString>>>,
    pub url_input: Entity<InputState>,
    pub headers_input: Entity<InputState>,
    pub params_input: Entity<InputState>,
    pub body_input: Entity<InputState>,
    /// The request as it was last read from or written to disk.
    ///
    /// The editors show name/value text only, so this is the base a draft is
    /// merged onto: it carries the fields the text cannot express — header
    /// descriptions, param types, a structured or multi-variant body — which
    /// would otherwise be dropped on the first edit.
    saved: HttpRequestDetails,
}

impl EventEmitter<TabDirtied> for RequestTab {}

impl RequestTab {
    pub fn new(
        target: TabTarget,
        title: String,
        saved: HttpRequestDetails,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let method_select = cx.new(|cx| {
            SelectState::new(
                METHODS
                    .into_iter()
                    .map(SharedString::from)
                    .collect::<Vec<_>>(),
                Some(IndexPath::default()),
                window,
                cx,
            )
        });
        let url_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("https://api.example.com/users"));
        let headers_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .placeholder("Accept: application/json")
        });
        let params_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .placeholder("page: 1")
        });
        let body_input = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("json")
                .line_number(true)
                .placeholder("{\n  \"name\": \"Ada\"\n}")
        });

        cx.subscribe_in(&url_input, window, Self::on_input_change)
            .detach();
        cx.subscribe_in(&headers_input, window, Self::on_input_change)
            .detach();
        cx.subscribe_in(&params_input, window, Self::on_input_change)
            .detach();
        cx.subscribe_in(&body_input, window, Self::on_input_change)
            .detach();
        cx.subscribe_in(&method_select, window, Self::on_method_change)
            .detach();

        let mut tab = Self {
            target,
            title,
            is_dirty: false,
            pane: RequestPane::Headers,
            response_pane: ResponsePane::Body,
            response_body_pretty: true,
            response: None,
            is_sending: false,
            method: "GET".to_string(),
            method_select,
            url_input,
            headers_input,
            params_input,
            body_input,
            saved: HttpRequestDetails::default(),
        };
        tab.reset_to(saved, window, cx);
        tab
    }

    /// Mirrors a saved request into the editors, discarding any draft.
    ///
    /// This is both how a tab is first filled and how a revert works, and it is
    /// the one path that clears `is_dirty` without writing anything.
    pub fn reset_to(
        &mut self,
        saved: HttpRequestDetails,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.method = saved
            .method
            .clone()
            .unwrap_or_else(|| "GET".to_string())
            .to_ascii_uppercase();

        let method = SharedString::from(self.method.clone());
        self.method_select.update(cx, |state, cx| {
            state.set_selected_value(&method, window, cx);
        });
        self.url_input.update(cx, |state, cx| {
            state.set_value(saved.url.clone().unwrap_or_default(), window, cx);
        });
        self.headers_input.update(cx, |state, cx| {
            state.set_value(
                format_headers(saved.headers.as_deref().unwrap_or_default()),
                window,
                cx,
            );
        });
        self.params_input.update(cx, |state, cx| {
            state.set_value(
                format_params(saved.params.as_deref().unwrap_or_default()),
                window,
                cx,
            );
        });
        self.body_input.update(cx, |state, cx| {
            let body = saved
                .body
                .as_ref()
                .and_then(crate::http_client::selected_body);
            state.set_highlighter(body_language(body), cx);
            state.set_value(body_text(body).to_string(), window, cx);
        });

        self.saved = saved;
        // Safe to clear: `set_value` and `set_selected_index` both suppress the
        // change events they would otherwise emit, so filling the editors here
        // does not trip the subscriptions that mark the tab dirty.
        self.is_dirty = false;
        cx.notify();
    }

    /// The edited request: the saved one with the editors' text merged in.
    pub fn draft(&self, cx: &App) -> HttpRequestDetails {
        let mut details = self.saved.clone();
        details.method = Some(self.method.clone());
        details.url = Some(self.url_input.read(cx).value().to_string());
        details.headers = Some(parse_headers(
            &self.headers_input.read(cx).value(),
            details.headers.as_deref(),
        ));
        details.params = Some(parse_params(
            &self.params_input.read(cx).value(),
            details.params.as_deref(),
        ));
        set_body_text(
            &mut details.body,
            self.body_input.read(cx).value().to_string(),
        );
        details
    }

    /// Marks the draft as written, so later saves start from what is on disk.
    pub fn mark_saved(&mut self, saved: HttpRequestDetails, cx: &mut Context<Self>) {
        self.saved = saved;
        self.is_dirty = false;
        cx.notify();
    }

    fn on_input_change(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            self.mark_dirty(cx);
        }
    }

    fn on_method_change(
        &mut self,
        _: &Entity<SelectState<Vec<SharedString>>>,
        event: &SelectEvent<Vec<SharedString>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let SelectEvent::Confirm(Some(value)) = event {
            self.method = value.to_string();
            self.mark_dirty(cx);
        }
    }

    fn mark_dirty(&mut self, cx: &mut Context<Self>) {
        if !self.is_dirty {
            self.is_dirty = true;
            cx.emit(TabDirtied);
        }
        cx.notify();
    }
}

/// Splits the `Name: value` lines the header and param editors use.
fn parse_lines(text: &str) -> impl Iterator<Item = (String, String)> + '_ {
    text.lines().filter_map(|line| {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let (name, value) = line.split_once(':').unwrap_or((line, ""));
        Some((name.trim().to_string(), value.trim().to_string()))
    })
}

/// Rebuilds the header list from editor text.
///
/// The text format carries only name and value, so fields it cannot express —
/// descriptions, the disabled flag — are carried over from the header of the
/// same name rather than dropped.
fn parse_headers(text: &str, existing: Option<&[HttpRequestHeader]>) -> Vec<HttpRequestHeader> {
    let existing = existing.unwrap_or_default();
    parse_lines(text)
        .map(|(name, value)| {
            let previous = existing.iter().find(|header| header.name == name);
            HttpRequestHeader {
                name,
                value,
                description: previous.and_then(|header| header.description.clone()),
                disabled: previous.and_then(|header| header.disabled),
            }
        })
        .collect()
}

/// Rebuilds the param list from editor text, preserving each param's type
/// (a path param stays a path param) along with its description and flag.
fn parse_params(text: &str, existing: Option<&[HttpRequestParam]>) -> Vec<HttpRequestParam> {
    let existing = existing.unwrap_or_default();
    parse_lines(text)
        .map(|(name, value)| {
            let previous = existing.iter().find(|param| param.name == name);
            HttpRequestParam {
                name,
                value,
                description: previous.and_then(|param| param.description.clone()),
                param_type: previous.map_or(ParamType::Query, |param| param.param_type),
                disabled: previous.and_then(|param| param.disabled),
            }
        })
        .collect()
}

pub fn format_headers(headers: &[HttpRequestHeader]) -> String {
    join_lines(headers.iter().map(|header| (&header.name, &header.value)))
}

pub fn format_params(params: &[HttpRequestParam]) -> String {
    join_lines(params.iter().map(|param| (&param.name, &param.value)))
}

pub fn join_lines<'a>(rows: impl Iterator<Item = (&'a String, &'a String)>) -> String {
    rows.filter(|(name, _)| !name.trim().is_empty())
        .map(|(name, value)| format!("{name}: {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn body_text(body: Option<&HttpRequestBody>) -> &str {
    match body {
        Some(
            HttpRequestBody::Json { data }
            | HttpRequestBody::Text { data }
            | HttpRequestBody::Xml { data }
            | HttpRequestBody::Sparql { data },
        ) => data,
        // Structured bodies have no text form; the editor shows them empty
        // rather than pretending to represent them.
        _ => "",
    }
}

fn body_language(body: Option<&HttpRequestBody>) -> &'static str {
    match body {
        Some(HttpRequestBody::Json { .. }) | None => "json",
        Some(HttpRequestBody::Xml { .. }) => "xml",
        _ => "text",
    }
}

/// Writes editor text back into the body, keeping the body's declared type.
///
/// The type is only inferred when there was no body to begin with: a request
/// the user marked as XML or SPARQL should not silently become text because
/// its content momentarily fails to parse.
fn set_body_text(body: &mut Option<HttpBodyOrVariants>, text: String) {
    if text.trim().is_empty() {
        *body = None;
        return;
    }

    let rebuild = |previous: &HttpRequestBody, data: String| match previous {
        HttpRequestBody::Xml { .. } => HttpRequestBody::Xml { data },
        HttpRequestBody::Sparql { .. } => HttpRequestBody::Sparql { data },
        HttpRequestBody::Text { .. } => HttpRequestBody::Text { data },
        _ => detect_body(data),
    };

    match body {
        None => *body = Some(detect_body(text).into()),
        Some(HttpBodyOrVariants::Body(previous)) => {
            *previous = rebuild(previous, text);
        }
        Some(HttpBodyOrVariants::Variants(variants)) => {
            let selected = variants
                .iter()
                .position(|variant| variant.selected == Some(true))
                .unwrap_or(0);
            if let Some(variant) = variants.get_mut(selected) {
                variant.body = rebuild(&variant.body, text);
            }
        }
    }
}

fn detect_body(data: String) -> HttpRequestBody {
    if serde_json::from_str::<serde_json::Value>(&data).is_ok() {
        HttpRequestBody::Json { data }
    } else {
        HttpRequestBody::Text { data }
    }
}

/// These cover what a draft carries back into the saved request. The editors
/// show name/value text only, so every field they cannot express has to survive
/// the round trip — otherwise saving one header edit would quietly strip the
/// descriptions and param types off the whole request.
#[cfg(test)]
mod tests {
    use super::*;
    use opencollection::Description;

    #[test]
    fn editing_a_header_keeps_the_fields_the_editor_cannot_show() {
        let existing = vec![HttpRequestHeader {
            name: "Accept".to_string(),
            value: "application/json".to_string(),
            description: Some(Description::Text("what we want back".to_string())),
            disabled: Some(true),
        }];

        let parsed = parse_headers("Accept: text/plain", Some(&existing));

        assert_eq!(parsed[0].value, "text/plain");
        assert_eq!(
            parsed[0].description,
            Some(Description::Text("what we want back".to_string()))
        );
        assert_eq!(parsed[0].disabled, Some(true));
    }

    #[test]
    fn a_path_param_stays_a_path_param_when_its_value_is_edited() {
        let existing = vec![HttpRequestParam {
            name: "id".to_string(),
            value: "1".to_string(),
            description: None,
            param_type: ParamType::Path,
            disabled: None,
        }];

        let parsed = parse_params("id: 2", Some(&existing));

        assert_eq!(parsed[0].value, "2");
        assert_eq!(parsed[0].param_type, ParamType::Path);
    }

    #[test]
    fn a_new_header_is_kept_alongside_the_edited_one() {
        let existing = vec![HttpRequestHeader {
            name: "Accept".to_string(),
            value: "application/json".to_string(),
            description: None,
            disabled: None,
        }];

        let parsed = parse_headers("Accept: application/json\nX-Trace: abc", Some(&existing));

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[1].name, "X-Trace");
        assert_eq!(parsed[1].description, None);
    }

    #[test]
    fn editing_an_xml_body_does_not_retype_it_as_text() {
        let mut body = Some(HttpBodyOrVariants::Body(HttpRequestBody::Xml {
            data: "<a/>".to_string(),
        }));

        set_body_text(&mut body, "<b/>".to_string());

        assert!(matches!(
            body,
            Some(HttpBodyOrVariants::Body(HttpRequestBody::Xml { .. }))
        ));
    }

    #[test]
    fn a_body_typed_from_scratch_is_detected() {
        let mut body = None;
        set_body_text(&mut body, "{\"a\": 1}".to_string());
        assert!(matches!(
            body,
            Some(HttpBodyOrVariants::Body(HttpRequestBody::Json { .. }))
        ));

        let mut body = None;
        set_body_text(&mut body, "not json".to_string());
        assert!(matches!(
            body,
            Some(HttpBodyOrVariants::Body(HttpRequestBody::Text { .. }))
        ));
    }

    #[test]
    fn clearing_the_body_editor_removes_the_body() {
        let mut body = Some(HttpBodyOrVariants::Body(HttpRequestBody::Json {
            data: "{}".to_string(),
        }));
        set_body_text(&mut body, "   ".to_string());
        assert!(body.is_none());
    }
}
