use gal_bindings::*;
use yew::prelude::*;

#[export]
fn plugin_type() -> PluginType {
    PluginType::HTML
}

#[function_component]
fn App() -> Html {
    let counter = use_state(|| 0);
    let onclick = {
        let counter = counter.clone();
        move |_| {
            let value = *counter + 1;
            counter.set(value);
        }
    };

    html! {
        <div>
            <button {onclick}>{ "+1" }</button>
            <p>{ *counter }</p>
        </div>
    }
}

#[export]
fn render_page() -> String {
    wasm_rs_async_executor::single_threaded::block_on(
        yew::LocalServerRenderer::<App>::new().render(),
    )
}
