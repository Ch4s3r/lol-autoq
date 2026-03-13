use dioxus::prelude::*;

#[component]
pub fn ChampionTile(
    name: String,
    alias: String,
    ddragon_version: String,
    selected: bool,
    on_click: EventHandler<String>,
) -> Element {
    let portrait_url = format!(
        "https://ddragon.leagueoflegends.com/cdn/{ddragon_version}/img/champion/{alias}.png"
    );
    let tile_class = if selected { "champ-tile selected" } else { "champ-tile" };
    let name_click = name.clone();

    rsx! {
        div {
            class: "{tile_class}",
            onclick: move |_| on_click.call(name_click.clone()),

            img {
                class: "champ-portrait",
                src: "{portrait_url}",
                alt: "{name}",
            }
            span { class: "champ-tile-name", "{name}" }

            if selected {
                div { class: "tile-check", i { class: "fa-solid fa-check" } }
            }
        }
    }
}
