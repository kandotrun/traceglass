mod ui;

use gpui_kit::{component::theme::{Theme, ThemeMode}, *};
use std::{borrow::Cow, cell::RefCell, rc::Rc, sync::Arc};
use wasm_bindgen::prelude::*;

gpui_kit::assets::icon_assets!(AppAssets, [Close, Search, Check, ChevronDown, ChevronRight]);

thread_local! {
    static APPLICATION: RefCell<Option<ApplicationHandle>> = const { RefCell::new(None) };
    static EXPLORER: RefCell<Option<(AnyWindowHandle, Entity<ui::Explorer>)>> = const { RefCell::new(None) };
}

/// Boot the real GPUI browser platform. The browser supplies the canvas and GPU.
#[wasm_bindgen]
pub fn run() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Warn);
    gpui_kit::platform::web_init();
    let platform = Rc::new(gpui_kit::web::WebPlatform::new_with_backend_and_font_fallback(
        false,
        gpui_kit::web::WebBackendPreference::Auto,
        gpui_kit::web::CanvasFontFallback::EmojiAndCjk,
    ));
    let http = Arc::new(platform.fetch_http_client());
    let app = Application::with_platform(platform).with_http_client(http).with_assets(AppAssets);
    APPLICATION.with(|application| {
        *application.borrow_mut() = Some(app.run_embedded(|cx| {
            gpui_kit::init(cx);
            cx.text_system().add_fonts(vec![
                Cow::Borrowed(include_bytes!("../fonts/IBMPlexSans-Regular.ttf").as_slice()),
                Cow::Borrowed(include_bytes!("../fonts/JetBrainsMono-Regular.ttf").as_slice()),
            ]).expect("bundled fonts");
            Theme::change(ThemeMode::Light, None, cx);
            Theme::update(cx, |theme| {
                theme.font_family = "IBM Plex Sans".into();
                theme.mono_font_family = "JetBrains Mono".into();
            });
            let pair = gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| ui::Explorer::new(window, cx))
            }).expect("browser window");
            EXPLORER.with(|explorer| *explorer.borrow_mut() = Some(pair));
            cx.activate(true);
        }));
    });
    Ok(())
}

/// Shared commands for the canvas UI, local file picker and accessible text view.
#[wasm_bindgen]
pub fn command(action: &str, value: &str) -> Result<(), JsValue> {
    APPLICATION.with(|application| {
        let borrow = application.borrow();
        let app = borrow.as_ref().ok_or_else(|| JsValue::from_str("Application is starting"))?;
        app.update(|cx| {
            EXPLORER.with(|explorer| {
                let pair = explorer.borrow();
                let (window, view) = pair.as_ref().ok_or_else(|| JsValue::from_str("Graphics are starting"))?;
                window.update(cx, |_, window, cx| {
                    view.update(cx, |this, cx| this.command(action, value, window, cx))
                }).map_err(|e| JsValue::from_str(&e.to_string()))?
                  .map_err(|e| JsValue::from_str(&e))
            })
        })
    })
}

/// A bounded snapshot of actual application state for accessibility and diagnostics.
#[wasm_bindgen]
pub fn snapshot() -> String {
    APPLICATION.with(|application| {
        let app = application.borrow();
        let Some(app) = app.as_ref() else { return "{}".into(); };
        app.update(|cx| {
            EXPLORER.with(|explorer| {
                explorer.borrow().as_ref()
                    .map(|(_, view)| view.read(cx).snapshot().to_string())
                    .unwrap_or_else(|| "{}".into())
            })
        })
    })
}
