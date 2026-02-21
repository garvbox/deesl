use leptos::prelude::*;

#[component]
pub fn App() -> impl IntoView {
    let (logged_in, set_logged_in) = signal(false);

    view! {
        <div class="app">
            <header>
                <h1>"Deesl Fuel Tracker"</h1>
                <Show when=move || logged_in.get() fallback=|| view! { <span>"Please log in"</span> }>
                    <button on:click=move |_| set_logged_in.set(false)>"Logout"</button>
                </Show>
            </header>
            <main>
                <Show when=move || logged_in.get() fallback=move || view! { <Login on_login=move || set_logged_in.set(true) /> }>
                    <Dashboard />
                </Show>
            </main>
        </div>
    }
}

#[component]
pub fn Login<F>(on_login: F) -> impl IntoView
where
    F: Fn() + 'static,
{
    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let error = RwSignal::new(String::new());

    view! {
        <div class="login">
            <h2>"Login"</h2>
            <form on:submit=move |ev| {
                ev.prevent_default();
                // TODO: Implement login
                on_login();
            }>
                <label>
                    "Email"
                    <input
                        type="email"
                        on:input=move |ev| email.set(event_target_value(&ev))
                    />
                </label>
                <label>
                    "Password"
                    <input
                        type="password"
                        on:input=move |ev| password.set(event_target_value(&ev))
                    />
                </label>
                <button type="submit">"Login"</button>
            </form>
            <p class="error">{error.get()}</p>
        </div>
    }
}

#[component]
pub fn Dashboard() -> impl IntoView {
    view! {
        <div class="dashboard">
            <h2>"Your Vehicles"</h2>
            <p>"Vehicle list coming soon"</p>

            <h2>"Quick Fuel Entry"</h2>
            <p>"Fuel entry form coming soon"</p>
        </div>
    }
}

pub fn main() {
    mount_to_body(App);
}
