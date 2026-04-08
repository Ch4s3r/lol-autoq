mod app_state;
mod champion_select;
mod config;
mod lcu;
mod logger;
mod ui;

fn main() {
    // Load config to get the persisted log level before setting up the subscriber.
    let initial_log_level = config::Config::load_or_create()
        .map(|c| c.log_level)
        .unwrap_or_else(|_| "info".to_string());

    // Open the log file in append mode and write a session separator.
    logger::init(&initial_log_level);

    // Tracing subscriber: fmt layer for stdout (respects RUST_LOG) +
    // our layer that writes to lol-autoq.log and the UI activity buffer.
    use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(env_filter))
        .with(logger::UiFileLayer)
        .init();

    let head = format!(
        r#"<link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.7.2/css/all.min.css"><style>{}</style>"#,
        ui::styles::CSS
    );

    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            dioxus::desktop::Config::new()
                .with_custom_head(head)
                .with_menu(None)
                .with_window(
                    dioxus::desktop::WindowBuilder::new()
                        .with_title("LoL AutoQ")
                        .with_inner_size(dioxus::desktop::tao::dpi::LogicalSize::new(
                            440.0_f64, 720.0_f64,
                        ))
                        .with_min_inner_size(dioxus::desktop::tao::dpi::LogicalSize::new(
                            360.0_f64, 600.0_f64,
                        )),
                ),
        )
        .launch(ui::App);
}
