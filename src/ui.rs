//! The GPUI view. Every displayed event and count comes from the Rust dataset.
use event_core::{Dataset, Level, Query, QueryResult};
use gpui_kit::{component::{button::*, input::*, *}, prelude::*, *};
use serde_json::{json, Value};
use gpui_kit::component::{input::InputEvent, scroll::ScrollableElement};

const CLOUD: u32 = 0xF4F6FB;
const PAPER: u32 = 0xFFFFFF;
const INK: u32 = 0x20283E;
const SLATE: u32 = 0x65728B;
const IRIS: u32 = 0x5964D9;
const ROSE: u32 = 0xC54461;
const AMBER: u32 = 0xB46C15;
const LINE: u32 = 0xE4E8F1;
const SOFT_IRIS: u32 = 0xEEF0FF;
const ROW_HEIGHT: f32 = 42.;

pub struct Explorer {
    dataset: Dataset,
    result: QueryResult,
    baseline: [u32; 80],
    errors: [u32; 80],
    input: Entity<InputState>,
    query_text: String,
    level: Option<Level>,
    service: Option<u16>,
    bucket: Option<usize>,
    slow: bool,
    selected: Option<usize>,
    mobile_details: bool,
    query_ms: Option<f64>,
    source: String,
    version: u64,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl Explorer {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dataset = Dataset::demo(100_000);
        let result = dataset.query(&Query::default());
        let baseline = result.histogram;
        let errors = dataset.query(&Query { level: Some(Level::Error), ..Query::default() }).histogram;
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search messages, services, or levels..."));
        let subscription = cx.subscribe_in(&input, window, |this, state, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                let text = state.read(cx).value().to_string();
                if text != this.query_text {
                    this.query_text = text;
                    this.run_query(cx);
                }
            }
        });
        let mut this = Self {
            dataset, result, baseline, errors, input,
            query_text: String::new(), level: None, service: None, bucket: None,
            slow: false, selected: Some(0), mobile_details: false, query_ms: None,
            source: "Simulated agent run".into(), version: 0,
            focus: cx.focus_handle(), scroll: UniformListScrollHandle::new(),
            _subscriptions: vec![subscription],
        };
        this.run_query(cx);
        this.focus.focus(window, cx);
        this
    }

    fn run_query(&mut self, cx: &mut Context<Self>) {
        let (start_ms, end_ms) = self.bucket_bounds();
        let query = Query {
            text: &self.query_text, level: self.level, service: self.service,
            start_ms, end_ms, min_duration_ms: if self.slow { 500 } else { 0 },
        };
        #[cfg(target_arch = "wasm32")]
        let timer = web_sys::window().and_then(|window| window.performance());
        #[cfg(target_arch = "wasm32")]
        let start = timer.as_ref().map(|timer| timer.now());
        #[cfg(not(target_arch = "wasm32"))]
        let start = std::time::Instant::now();
        self.result = self.dataset.query(&query);
        #[cfg(target_arch = "wasm32")]
        { self.query_ms = start.zip(timer.as_ref()).map(|(start, timer)| (timer.now() - start).max(0.)); }
        #[cfg(not(target_arch = "wasm32"))]
        { self.query_ms = Some(start.elapsed().as_secs_f64() * 1_000.); }
        self.selected = if self.result.indices.is_empty() { None } else { Some(0) };
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.version += 1;
        cx.notify();
    }

    fn bucket_bounds(&self) -> (Option<u64>, Option<u64>) {
        match (self.bucket, self.dataset.time_bounds()) {
            (Some(bucket), Some((start, end))) => {
                let span = end.saturating_sub(start);
                if span == 0 {
                    return if bucket == 0 { (Some(start), Some(end)) } else { (Some(1), Some(0)) };
                }
                // ceil(span * bucket / 80) is the inverse of the histogram's floor mapping.
                let lo = start + ((u128::from(span) * bucket as u128 + 79) / 80) as u64;
                let hi = start + ((u128::from(span) * (bucket + 1) as u128 + 79) / 80) as u64;
                (Some(lo), Some(if bucket == 79 { end } else { hi.saturating_sub(1) }))
            }
            _ => (None, None),
        }
    }

    fn replace_dataset(&mut self, dataset: Dataset, source: String, window: &mut Window, cx: &mut Context<Self>) {
        self.dataset = dataset;
        self.source = source;
        self.baseline = self.dataset.query(&Query::default()).histogram;
        self.errors = self.dataset.query(&Query { level: Some(Level::Error), ..Query::default() }).histogram;
        self.clear_filters(window, cx);
    }

    fn clear_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.query_text.clear();
        self.level = None;
        self.service = None;
        self.bucket = None;
        self.slow = false;
        self.mobile_details = false;
        self.input.update(cx, |input, cx| input.set_value("", window, cx));
        self.run_query(cx);
    }

    /// Browser controls and the accessible text view drive the very same state as GPUI.
    pub fn command(&mut self, action: &str, value: &str, window: &mut Window, cx: &mut Context<Self>) -> Result<(), String> {
        match action {
            "search" => {
                self.query_text = value.to_owned();
                self.input.update(cx, |input, cx| input.set_value(value.to_owned(), window, cx));
                self.run_query(cx);
            }
            "level" => {
                self.level = match value {
                    "all" => None, "debug" => Some(Level::Debug), "info" => Some(Level::Info),
                    "warn" => Some(Level::Warn), "error" => Some(Level::Error),
                    _ => return Err("Choose all, debug, info, warn, or error.".into()),
                };
                self.run_query(cx);
            }
            "service" => {
                self.service = if value == "all" { None } else {
                    let index: u16 = value.parse().map_err(|_| "Service must be all or a numeric index.")?;
                    if usize::from(index) >= self.dataset.services().len() { return Err("Unknown service index.".into()); }
                    Some(index)
                };
                self.run_query(cx);
            }
            "bucket" => {
                self.bucket = if value == "all" { None } else {
                    let bucket = value.parse::<usize>().map_err(|_| "Time bin must be all or 0-79.")?;
                    if bucket >= 80 { return Err("Time bin must be between 0 and 79.".into()); }
                    Some(bucket)
                };
                self.run_query(cx);
            }
            "slow" => {
                self.slow = match value { "on" => true, "off" => false, _ => return Err("Slow filter must be on or off.".into()) };
                self.run_query(cx);
            }
            "demo" => {
                let count = value.parse::<usize>().map_err(|_| "Choose the 100000 or 1000000 event demo.")?;
                if !matches!(count, 100_000 | 1_000_000) { return Err("Choose the 100000 or 1000000 event demo.".into()); }
                self.replace_dataset(Dataset::demo(count), "Simulated agent run".into(), window, cx);
            }
            "inspect" => {
                let position = value.parse::<usize>().map_err(|_| "Event position must be a number.")?;
                if position >= self.result.indices.len() { return Err("Event position is outside the current results.".into()); }
                self.selected = Some(position);
                self.mobile_details = true;
                self.scroll.scroll_to_item(position, ScrollStrategy::Center);
                self.version += 1;
                cx.notify();
            }
            "import" => {
                let dataset = Dataset::import_jsonl(value).map_err(|error| error.to_string())?;
                self.replace_dataset(dataset, "Imported JSONL / local file".into(), window, cx);
            }
            "reset" => self.clear_filters(window, cx),
            "focus" => self.input.update(cx, |input, cx| input.focus(window, cx)),
            _ => return Err(format!("Unknown command: {action}")),
        }
        Ok(())
    }

    pub fn snapshot(&self) -> Value {
        let row = |position: usize| -> Option<Value> {
            let index = *self.result.indices.get(position)?;
            let event = self.dataset.event(index)?;
            let display = self.dataset.display(index)?;
            Some(json!({
                "index": index, "position": position, "timestamp": clock_time(event.timestamp_ms),
                "timestamp_ms": event.timestamp_ms, "timestamp_utc": utc_timestamp(event.timestamp_ms), "message": display.message,
                "service": display.service, "level": event.level.as_str(),
                "duration_ms": event.duration_ms, "trace_id": event.trace_id,
            }))
        };
        let mut selected = self.selected.and_then(row);
        if let (Some(selected), Some(position)) = (selected.as_mut(), self.selected) {
            selected["details"] = self.result.indices.get(position)
                .and_then(|index| self.dataset.details(*index)).cloned().unwrap_or(Value::Null);
        }
        json!({
            "version": self.version, "total": self.dataset.len(), "matched": self.result.indices.len(),
            "query_ms": self.query_ms, "query_text": self.query_text,
            "level": self.level.map(|level| level.as_str().to_ascii_lowercase()).unwrap_or_else(|| "all".into()),
            "service": self.service, "services": self.dataset.services(), "bucket": self.bucket,
            "slow": self.slow, "selected": selected, "source": self.source,
            "first_rows": (0..self.result.indices.len().min(20)).filter_map(row).collect::<Vec<_>>(),
            "histogram": self.result.histogram.to_vec(), "overview_histogram": self.baseline.to_vec(),
            "error_histogram": self.errors.to_vec(), "level_counts": self.result.level_counts,
            "duration_sum_ms": self.result.duration_sum_ms, "max_duration_ms": self.result.max_duration_ms,
        })
    }

    fn render_header(&self, compact: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div().h(px(67.)).flex_shrink_0().px(if compact { px(16.) } else { px(25.) })
            .flex().items_center().justify_between().border_b_1().border_color(rgb(LINE)).bg(rgb(PAPER))
            .child(div().flex().items_center().gap_3()
                .child(div().w(px(27.)).h(px(30.)).flex().items_end().gap(px(3.))
                    .child(div().w(px(6.)).h(px(14.)).rounded(px(2.)).bg(rgb(IRIS)))
                    .child(div().w(px(6.)).h(px(28.)).rounded(px(2.)).bg(rgb(IRIS)))
                    .child(div().w(px(6.)).h(px(20.)).rounded(px(2.)).bg(rgb(0xA8AFF1))))
                .child(div().flex().flex_col().gap(px(1.))
                    .child(div().text_size(px(21.)).font_weight(FontWeight::SEMIBOLD).child("Traceglass"))
                    .child(div().text_size(px(10.)).text_color(rgb(SLATE)).child("Agent flight recorder"))))
            .child(div().flex().items_center().gap_3()
                .when(!compact, |this| this.child(div().flex().items_center().gap_2().text_size(px(11.)).text_color(rgb(SLATE))
                    .child(div().size(px(6.)).rounded_full().bg(rgb(IRIS))).child("Browser workbench")))
                .child(Button::new("import-jsonl").label(if compact { "Import" } else { "Import JSONL" }).outline()
                    .on_click(cx.listener(|_, _, _, _| open_file_picker()))))
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let is_demo = self.source == "Simulated agent run";
        div().w(px(190.)).flex_shrink_0().h_full().border_r_1().border_color(rgb(LINE)).bg(rgb(0xF8F9FC))
            .flex().flex_col().p_4().gap_5()
            .child(div().flex().flex_col().gap_2()
                .child(eyebrow("Workspace"))
                .child(div().rounded(px(6.)).bg(rgb(SOFT_IRIS)).text_color(rgb(IRIS)).px_3().py_2().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child("Event explorer")))
            .child(div().flex().flex_col().gap_2()
                .child(eyebrow("Data source"))
                .child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(if is_demo { "Agent run / demo" } else { "Your local JSONL" }))
                .child(div().text_size(px(11.)).text_color(rgb(SLATE)).child(format!("{} events loaded", grouped(self.dataset.len()))))
                .child(div().flex().gap_1().pt_1()
                    .child(Button::new("demo-100k").label("100K").small().selected(is_demo && self.dataset.len() == 100_000)
                        .on_click(cx.listener(|this, _, window, cx| { let _ = this.command("demo", "100000", window, cx); })))
                    .child(Button::new("demo-1m").label("1M").small().selected(is_demo && self.dataset.len() == 1_000_000)
                        .on_click(cx.listener(|this, _, window, cx| { let _ = this.command("demo", "1000000", window, cx); })))))
            .child(div().flex_1().min_h_0().flex().flex_col().gap_2()
                .child(eyebrow("Services"))
                .child(div().id("services-scroll").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().gap_1()
                    .child(self.service_button(None, "All services".into(), cx))
                    .children(self.dataset.services().iter().enumerate().map(|(index, name)| self.service_button(Some(index as u16), name.clone(), cx)))))
            .child(div().flex().flex_col().gap_2().border_t_1().border_color(rgb(LINE)).pt_4()
                .child(div().text_size(px(11.)).font_weight(FontWeight::MEDIUM).child("Your data stays here."))
                .child(div().text_size(px(11.)).text_color(rgb(SLATE)).child("Queries and imported files run in this browser.")))
    }

    fn service_button(&self, service: Option<u16>, label: String, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let active = self.service == service;
        div().id(("service", service.map_or(0, |index| usize::from(index) + 1)))
            .w_full().rounded(px(5.)).px_2().h(px(31.)).flex().items_center().gap_2()
            .cursor_pointer().text_size(px(12.)).text_color(rgb(if active { IRIS } else { SLATE }))
            .bg(rgb(if active { SOFT_IRIS } else { 0xF8F9FC }))
            .hover(|style| style.bg(rgb(0xEEF0F7)))
            .child(div().size(px(5.)).flex_shrink_0().rounded_full().bg(rgb(if active { IRIS } else { 0xB7BFD0 })))
            .child(div().truncate().child(label))
            .on_click(cx.listener(move |this, _, _, cx| { this.service = service; this.run_query(cx); }))
    }

    fn render_controls(&self, compact: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let levels = [(None, "All"), (Some(Level::Error), "Error"), (Some(Level::Warn), "Warn"), (Some(Level::Info), "Info"), (Some(Level::Debug), "Debug")];
        div().flex().flex_col().gap_3().px(if compact { px(16.) } else { px(24.) }).pt(px(21.)).pb(px(16.))
            .child(div().flex().items_start().justify_between().gap_3()
                .child(div().flex().flex_col().gap_1()
                    .child(div().text_size(px(if compact { 22. } else { 25. })).font_weight(FontWeight::SEMIBOLD).child("Find the moment."))
                    .child(div().text_size(px(12.)).text_color(rgb(SLATE)).child(if compact { "Search a run. Inspect an event." } else { "Search a run. Inspect an event. Understand what happened." })))
                .when(!compact, |this| this.child(div().text_size(px(10.)).text_color(rgb(SLATE)).pt_2().child(self.source.clone()))))
            .child(div().flex().items_center().gap_2()
                .child(div().flex_1().min_w_0().child(Input::new(&self.input).id("event-search").cleanable(true)
                    .aria_label("Search event messages, services, and levels")
                    .prefix(Icon::new(IconName::Search).small().text_color(rgb(SLATE)))))
                .child(Button::new("reset-filters").label("Reset").ghost().on_click(cx.listener(|this, _, window, cx| this.clear_filters(window, cx)))))
            .child(div().flex().items_center().justify_between().gap_2()
                .child(div().flex().gap(px(5.)).children(levels.map(|(level, label)| {
                    let active = self.level == level;
                    let color = level.map(level_color).unwrap_or(IRIS);
                    div().id(("level", level.map_or(0usize, |level| level as usize + 1))).px(if compact { px(8.) } else { px(11.) }).h(px(28.)).rounded(px(5.))
                        .flex().items_center().justify_center().gap(px(5.)).cursor_pointer().text_size(px(11.))
                        .border_1().border_color(rgb(if active { color } else { LINE }))
                        .bg(rgb(if active { level.map(level_soft).unwrap_or(SOFT_IRIS) } else { PAPER }))
                        .text_color(rgb(if active { color } else { SLATE }))
                        .when(level.is_some(), |this| this.child(div().size(px(5.)).rounded_full().bg(rgb(color))))
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| { this.level = level; this.run_query(cx); }))
                })))
                .child(div().id("slow-filter").h(px(28.)).px_2().rounded(px(5.)).cursor_pointer().flex().items_center().text_size(px(11.))
                    .bg(rgb(if self.slow { 0xFFF2DE } else { CLOUD })).text_color(rgb(if self.slow { AMBER } else { SLATE }))
                    .child(if compact { "500 ms+" } else { "Slow / 500 ms+" })
                    .on_click(cx.listener(|this, _, _, cx| { this.slow = !this.slow; this.run_query(cx); }))))
            .when(compact, |this| this.child(div().flex().items_center().justify_between().gap_2()
                .child(div().flex().gap_1()
                    .child(Button::new("compact-demo-100k").small().label("100K demo").on_click(cx.listener(|this, _, window, cx| { let _ = this.command("demo", "100000", window, cx); })))
                    .child(Button::new("compact-demo-1m").small().label("1M demo").on_click(cx.listener(|this, _, window, cx| { let _ = this.command("demo", "1000000", window, cx); }))))
                .child(Button::new("compact-service").small().label(self.service
                    .and_then(|index| self.dataset.services().get(usize::from(index))).cloned().unwrap_or_else(|| "All services".into())).icon(IconName::ChevronRight)
                    .tooltip("Click to cycle through services")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.service = match this.service {
                            None if !this.dataset.services().is_empty() => Some(0),
                            Some(index) if usize::from(index) + 1 < this.dataset.services().len() => Some(index + 1),
                            _ => None,
                        };
                        this.run_query(cx);
                    })))))
    }

    fn render_timeline(&self, compact: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let max = self.baseline.iter().copied().max().unwrap_or(1).max(1) as f32;
        let bounds = self.dataset.time_bounds();
        let selected_label = self.bucket.map(|bucket| format!("Bin {:02} / 80 / clear", bucket + 1));
        let height = if compact { 49. } else { 66. };
        div().px(if compact { px(16.) } else { px(24.) }).pb_4().flex().flex_col().gap_2()
            .child(div().flex().justify_between().items_center()
                .child(div().flex().items_center().gap_2().child(eyebrow("Run timeline"))
                    .child(div().text_size(px(10.)).text_color(rgb(SLATE)).child("80 bins")))
                .child(div().id("clear-timeline").cursor_pointer().text_size(px(10.)).text_color(rgb(if self.bucket.is_some() { IRIS } else { SLATE }))
                    .child(selected_label.unwrap_or_else(|| "Click a bar to isolate a moment".into()))
                    .on_click(cx.listener(|this, _, _, cx| { this.bucket = None; this.run_query(cx); }))))
            .child(div().h(px(height + 6.)).flex().items_end().gap(px(2.)).children((0..80).map(|index| {
                let total = self.baseline[index];
                let matched = self.result.histogram[index];
                let error = self.errors[index];
                let baseline_height = if total == 0 { 2. } else { (total as f32 / max * height).max(3.) };
                let matched_height = if matched == 0 { 0. } else { (matched as f32 / max * height).max(2.) };
                let error_height = if error == 0 { 0. } else { (error as f32 / max * height).max(2.) };
                div().id(("time-bin", index)).flex_1().min_w(px(1.)).h_full().relative().cursor_pointer()
                    .rounded_t(px(2.)).bg(rgb(if self.bucket == Some(index) { SOFT_IRIS } else { PAPER }))
                    .hover(|style| style.bg(rgb(SOFT_IRIS)))
                    .child(div().absolute().bottom_0().w_full().h(px(baseline_height)).rounded_t(px(2.)).bg(rgb(0xE1E5F1)))
                    .when(matched_height > 0., |this| this.child(div().absolute().bottom_0().w_full().h(px(matched_height)).rounded_t(px(2.))
                        .bg(rgb(if self.bucket.is_none() || self.bucket == Some(index) { 0x9BA4E5 } else { 0xCCD1ED }))))
                    .when(error_height > 0., |this| this.child(div().absolute().bottom_0().w_full().h(px(error_height)).rounded_t(px(1.)).bg(rgb(ROSE))))
                    .on_click(cx.listener(move |this, _, _, cx| { this.bucket = if this.bucket == Some(index) { None } else { Some(index) }; this.run_query(cx); }))
            })))
            .child(div().flex().justify_between().items_center().text_size(px(10.)).text_color(rgb(SLATE))
                .child(div().font_family("JetBrains Mono").child(bounds.map(|(start, _)| clock_short(start)).unwrap_or_else(|| "-".into())))
                .child(div().flex().items_center().gap_3()
                    .child(div().flex().items_center().gap_1().child(div().size(px(5.)).rounded_full().bg(rgb(0x9BA4E5))).child("events"))
                    .child(div().flex().items_center().gap_1().child(div().size(px(5.)).rounded_full().bg(rgb(ROSE))).child("errors")))
                .child(div().font_family("JetBrains Mono").child(bounds.map(|(_, end)| clock_short(end)).unwrap_or_else(|| "-".into()))))
    }

    fn render_results(&self, compact: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let view = cx.entity().clone();
        div().flex_1().min_h_0().min_w_0().flex().flex_col().overflow_hidden()
            .child(div().h(px(45.)).flex_shrink_0().px(if compact { px(16.) } else { px(24.) }).border_t_1().border_b_1().border_color(rgb(LINE))
                .flex().items_center().justify_between().gap_2().bg(rgb(0xFCFCFE))
                .child(div().flex().items_baseline().gap_2()
                    .child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child(grouped(self.result.indices.len())))
                    .child(div().text_size(px(11.)).text_color(rgb(SLATE)).child("matching events")))
                .child(div().flex().items_center().gap_1().text_size(px(10.)).text_color(rgb(SLATE))
                    .child(div().text_color(rgb(IRIS)).font_family("JetBrains Mono").child(self.query_ms.map(|ms| format!("{ms:.1} ms")).unwrap_or_else(|| "-".into())))
                    .child("Rust query")))
            .child(div().h(px(30.)).flex_shrink_0().px(if compact { px(16.) } else { px(24.) }).flex().items_center().gap_3().text_size(px(10.)).text_color(rgb(SLATE)).bg(rgb(PAPER))
                .child(div().w(px(if compact { 71. } else { 102. })).flex_shrink_0().child("Time / UTC"))
                .when(!compact, |this| this.child(div().w(px(47.)).flex_shrink_0().child("Level")))
                .child(div().flex_1().min_w_0().child("Event"))
                .child(div().w(px(54.)).text_right().child("Duration")))
            .child(if self.result.indices.is_empty() {
                div().flex_1().flex().flex_col().items_center().justify_center().gap_3().p_6()
                    .child(div().text_size(px(23.)).text_color(rgb(IRIS)).child("No matching moments"))
                    .child(div().text_size(px(13.)).text_color(rgb(SLATE)).child("Try another phrase, widen the timeline, or reset the filters."))
                    .child(Button::new("empty-reset").label("Reset filters").outline().on_click(cx.listener(|this, _, window, cx| this.clear_filters(window, cx))))
                    .into_any_element()
            } else {
                div().relative().flex_1().min_h_0().overflow_hidden()
                    .child(uniform_list("event-list", self.result.indices.len(), move |range, _, app| {
                        view.update(app, |this, cx| range.map(|position| this.render_row(position, compact, cx).into_any_element()).collect::<Vec<_>>())
                    }).size_full().track_scroll(&self.scroll))
                    .vertical_scrollbar(&self.scroll)
                    .into_any_element()
            })
    }

    fn render_row(&self, position: usize, compact: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let index = self.result.indices[position];
        let event = self.dataset.event(index).expect("query index belongs to the dataset");
        let display = self.dataset.display(index).expect("query index has display data");
        let selected = self.selected == Some(position);
        let color = level_color(event.level);
        div().id(("event", position)).h(px(ROW_HEIGHT)).w_full().px(if compact { px(16.) } else { px(24.) })
            .flex().items_center().gap_3().border_b_1().border_color(rgb(0xF0F2F7))
            .bg(rgb(if selected { SOFT_IRIS } else if position % 2 == 0 { PAPER } else { 0xFCFCFE }))
            .cursor_pointer().hover(|style| style.bg(rgb(0xF1F3FC)))
            .child(div().w(px(if compact { 71. } else { 102. })).flex_shrink_0().font_family("JetBrains Mono").text_size(px(10.)).text_color(rgb(SLATE))
                .child(if compact { clock_short(event.timestamp_ms) } else { clock_time(event.timestamp_ms) }))
            .when(!compact, |this| this.child(div().w(px(47.)).flex_shrink_0().flex().items_center().gap(px(4.)).text_size(px(9.)).text_color(rgb(color))
                .child(div().size(px(4.)).rounded_full().bg(rgb(color))).child(event.level.as_str().to_uppercase())))
            .child(div().flex_1().min_w_0().flex().flex_col().gap(px(1.))
                .child(div().truncate().text_size(px(12.)).text_color(rgb(if event.level == Level::Error { ROSE } else { INK })).child(display.message))
                .child(div().truncate().text_size(px(9.)).text_color(rgb(SLATE)).child(format!("{}   /   trace {:08x}", display.service, event.trace_id))))
            .child(div().w(px(54.)).flex_shrink_0().text_right().font_family("JetBrains Mono").text_size(px(10.)).text_color(rgb(if event.duration_ms >= 500 { AMBER } else { SLATE })).child(format!("{} ms", event.duration_ms)))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.selected = Some(position);
                this.mobile_details = true;
                this.focus.focus(window, cx);
                this.version += 1;
                cx.notify();
            }))
    }

    fn render_inspector(&self, stacked: bool, compact: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let current = self.selected.and_then(|position| self.result.indices.get(position)).and_then(|index| {
            self.dataset.event(*index).zip(self.dataset.display(*index)).map(|(event, display)| (*index, event, display))
        });
        div().id("event-inspector").flex_shrink_0().bg(rgb(0xFAFBFE)).border_color(rgb(LINE))
            .when(stacked, |this| this.w_full().h(px(180.)).border_t_1())
            .when(!stacked, |this| this.w(px(302.)).h_full().border_l_1())
            .overflow_y_scroll().flex().flex_col().p(if stacked { px(16.) } else { px(22.) }).gap_4()
            .child(div().flex().items_center().justify_between().child(eyebrow("Event inspector"))
                .child(div().text_size(px(10.)).text_color(rgb(SLATE)).child(self.selected.map(|position| format!("{:04}", position + 1)).unwrap_or_else(|| "-".into())))
                .when(compact, |this| this.child(Button::new("close-inspector").label("Close").small().ghost().on_click(cx.listener(|this, _, _, cx| { this.mobile_details = false; this.version += 1; cx.notify(); })))))
            .child(match current {
                Some((index, event, display)) => {
                    let details = self.dataset.details(index).and_then(|value| serde_json::to_string_pretty(value).ok());
                    div().flex().flex_col().gap_4()
                        .child(div().flex().items_center().gap_2()
                            .child(div().px_2().py_1().rounded(px(4.)).text_size(px(10.)).text_color(rgb(level_color(event.level))).bg(rgb(level_soft(event.level))).child(event.level.as_str().to_uppercase()))
                            .child(div().text_size(px(11.)).text_color(rgb(SLATE)).child(display.service.clone())))
                        .child(div().text_size(px(if stacked { 14. } else { 17. })).line_height(relative(1.4)).font_weight(FontWeight::MEDIUM).child(display.message))
                        .child(div().w_full().flex_shrink_0().border_t_1().border_b_1().border_color(rgb(LINE)).py_4()
                            .font_family("JetBrains Mono").text_size(px(13.)).line_height(relative(1.9)).text_color(rgb(SLATE))
                            .child(format!(
                                "Date / UTC   {}\nTime / UTC   {}\nDuration     {} ms\nTrace ID     {:08x}\nEvent index  {}",
                                utc_date(event.timestamp_ms), clock_time(event.timestamp_ms), event.duration_ms, event.trace_id, grouped(index)
                            )))
                        .child(div().flex().flex_col().gap_2()
                            .child(eyebrow(if details.is_some() { "Event payload" } else { "About this event" }))
                            .child(div().text_size(px(11.)).line_height(relative(1.55)).text_color(rgb(SLATE))
                                .when(details.is_some(), |this| this.font_family("JetBrains Mono"))
                                .child(details.unwrap_or_else(|| if self.source.starts_with("Simulated") {
                                    "Deterministic demo data. Select a rose timeline bar to inspect the failure burst, or search a message to find related events.".into()
                                } else {
                                    "Imported from your JSONL file. Original fields remain available in this inspector.".into()
                                }))))
                        .into_any_element()
                }
                None => div().flex().flex_col().gap_3().pt_6()
                    .child(div().text_size(px(17.)).font_weight(FontWeight::MEDIUM).child("Every event has a story."))
                    .child(div().text_size(px(12.)).line_height(relative(1.5)).text_color(rgb(SLATE)).child("Select a row to see the message, duration, trace, and original payload."))
                    .into_any_element(),
            })
    }

    fn render_footer(&self, compact: bool) -> impl IntoElement + use<> {
        div().h(px(35.)).flex_shrink_0().border_t_1().border_color(rgb(LINE)).bg(rgb(0xF8F9FC)).px_4().pr(px(115.))
            .flex().items_center().justify_between().gap_3().text_size(px(10.)).text_color(rgb(SLATE))
            .child(div().flex().items_center().gap_2()
                .child(div().size(px(5.)).rounded_full().bg(rgb(IRIS)))
                .child(if compact { "Rust / WebAssembly" } else { "Rust + GPUI Kit  /  WebAssembly" }))
            .when(!compact, |this| this.child("/ search   |   Up/Down inspect   |   Esc clear timeline"))
    }
}

impl Focusable for Explorer {
    fn focus_handle(&self, _: &App) -> FocusHandle { self.focus.clone() }
}

impl Render for Explorer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width: f32 = window.viewport_size().width.into();
        let compact = width < 760.;
        let stacked = width < 1080.;
        let show_inspector = !compact || self.mobile_details;
        div().id("traceglass").track_focus(&self.focus).size_full().overflow_hidden().flex().flex_col()
            .bg(rgb(CLOUD)).text_color(rgb(INK)).font_family("IBM Plex Sans").text_size(px(13.))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                let searching = this.input.read(cx).focus_handle(cx).is_focused(window);
                if (key == "/" && !searching) || (key == "f" && (event.keystroke.modifiers.platform || event.keystroke.modifiers.control)) {
                    this.input.update(cx, |input, cx| input.focus(window, cx));
                    cx.stop_propagation();
                } else if key == "escape" {
                    this.bucket = None;
                    this.focus.focus(window, cx);
                    this.run_query(cx);
                    cx.stop_propagation();
                } else if !searching && matches!(key, "up" | "down") && !this.result.indices.is_empty() {
                    let old = this.selected.unwrap_or(0);
                    let next = if key == "up" { old.saturating_sub(1) } else { (old + 1).min(this.result.indices.len() - 1) };
                    this.selected = Some(next);
                    this.mobile_details = true;
                    this.scroll.scroll_to_item(next, ScrollStrategy::Center);
                    this.version += 1;
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .child(self.render_header(compact, cx))
            .child(div().flex_1().min_h_0().min_w_0().flex().overflow_hidden()
                .when(!compact, |this| this.child(self.render_sidebar(cx)))
                .child(div().flex_1().min_w_0().min_h_0().flex().flex_col().bg(rgb(PAPER))
                    .child(self.render_controls(compact, cx))
                    .child(self.render_timeline(compact, cx))
                    .child(div().flex_1().min_h_0().min_w_0().flex().when(stacked, |this| this.flex_col())
                        .child(self.render_results(compact, cx))
                        .when(show_inspector, |this| this.child(self.render_inspector(stacked, compact, cx))))))
            .child(self.render_footer(compact))
    }
}

fn eyebrow(label: &'static str) -> impl IntoElement + use<> {
    div().text_size(px(10.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(SLATE)).child(label)
}

fn level_color(level: Level) -> u32 {
    match level { Level::Debug => 0x8290A8, Level::Info => IRIS, Level::Warn => AMBER, Level::Error => ROSE }
}

fn level_soft(level: Level) -> u32 {
    match level { Level::Debug => 0xEEF1F6, Level::Info => SOFT_IRIS, Level::Warn => 0xFFF2DE, Level::Error => 0xFBEAF0 }
}

fn utc_date(timestamp: u64) -> String {
    // Gregorian civil date from days since 1970-01-01, without host timezone state.
    let days = (timestamp / 86_400_000) as i64 + 719_468;
    let era = days / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

fn utc_timestamp(timestamp: u64) -> String { format!("{}T{}Z", utc_date(timestamp), clock_time(timestamp)) }

fn clock_time(timestamp: u64) -> String {
    let seconds = timestamp / 1_000;
    format!("{:02}:{:02}:{:02}.{:03}", (seconds / 3_600) % 24, (seconds / 60) % 60, seconds % 60, timestamp % 1_000)
}

fn clock_short(timestamp: u64) -> String {
    let seconds = timestamp / 1_000;
    format!("{:02}:{:02}:{:02}", (seconds / 3_600) % 24, (seconds / 60) % 60, seconds % 60)
}

fn grouped(value: usize) -> String {
    let digits = value.to_string();
    let mut result = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index != 0 && (digits.len() - index) % 3 == 0 { result.push(','); }
        result.push(digit);
    }
    result
}

fn open_file_picker() {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        if let Some(input) = web_sys::window().and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id("jsonl-file"))
            .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok()) {
            input.click();
        }
    }
}
