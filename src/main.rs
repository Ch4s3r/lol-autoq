mod app_state;
mod champion_select;
mod config;
mod lcu;
mod ui;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
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
                        .with_title("LoL Auto-Queue")
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

