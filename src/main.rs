mod app_state;
mod champion_select;
mod config;
mod lcu;
mod ui;

fn main() {
    let head = format!(
        concat!(
            "<meta http-equiv='Content-Security-Policy' content=\"",
            "default-src 'self' 'unsafe-inline' data: https://ddragon.leagueoflegends.com",
            "\">",
        )
    );

    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            dioxus::desktop::Config::new()
                .with_custom_head(head)
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

