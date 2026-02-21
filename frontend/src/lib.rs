mod auth;
mod fuel_entries;
mod vehicles;

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::auth::AuthResponse;
use crate::fuel_entries::{FuelEntry, FuelStation};
use crate::vehicles::Vehicle;

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub token: String,
    pub user_id: i32,
    pub email: String,
}

impl From<AuthResponse> for CurrentUser {
    fn from(auth: AuthResponse) -> Self {
        Self {
            token: auth.token,
            user_id: auth.user_id,
            email: auth.email,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AuthState {
    user: RwSignal<Option<CurrentUser>>,
}

impl Default for AuthState {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthState {
    pub fn new() -> Self {
        let stored = auth::get_stored_auth().map(CurrentUser::from);
        let user = RwSignal::new(stored);
        Self { user }
    }

    pub fn login(&self, auth_response: AuthResponse) {
        let user = CurrentUser::from(auth_response);
        auth::store_auth(&AuthResponse {
            token: user.token.clone(),
            user_id: user.user_id,
            email: user.email.clone(),
        });
        self.user.set(Some(user));
    }

    pub fn logout(&self) {
        auth::clear_auth();
        self.user.set(None);
    }

    pub fn is_logged_in(&self) -> bool {
        self.user.read().is_some()
    }

    pub fn user(&self) -> Option<CurrentUser> {
        self.user.read().clone()
    }
}

#[component]
pub fn App() -> impl IntoView {
    let auth_state = AuthState::new();

    view! {
        <div class="app">
            <header>
                <h1>"Deesl Fuel Tracker"</h1>
                <Show when=move || auth_state.is_logged_in() fallback=|| view! { <span>"Please log in"</span> }>
                    {move || {
                        if let Some(user) = auth_state.user() {
                            view! {
                                <span class="user-info">{user.email}</span>
                                <button on:click=move |_| auth_state.logout()>"Logout"</button>
                            }.into_any()
                        } else {
                            view! { <span></span> }.into_any()
                        }
                    }}
                </Show>
            </header>
            <main>
                <Show
                    when=move || auth_state.is_logged_in()
                    fallback=move || view! {
                        <AuthForms auth_state=auth_state />
                    }
                >
                    <Dashboard auth_state=auth_state />
                </Show>
            </main>
        </div>
    }
}

#[component]
pub fn AuthForms(auth_state: AuthState) -> impl IntoView {
    let (show_register, set_show_register) = signal(false);

    view! {
        <div class="auth-forms">
            <Show
                when=move || show_register.get()
                fallback=move || view! {
                    <Login
                        auth_state=auth_state
                        on_switch_to_register=move || set_show_register.set(true)
                    />
                }
            >
                <Register
                    auth_state=auth_state
                    on_switch_to_login=move || set_show_register.set(false)
                />
            </Show>
        </div>
    }
}

#[component]
pub fn Login(
    auth_state: AuthState,
    on_switch_to_register: impl Fn() + 'static + Copy,
) -> impl IntoView {
    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let error = RwSignal::new(String::new());
    let loading = RwSignal::new(false);

    view! {
        <div class="login">
            <h2>"Login"</h2>
            <form on:submit=move |ev| {
                ev.prevent_default();
                let email_val = email.get();
                let password_val = password.get();

                if email_val.is_empty() || password_val.is_empty() {
                    error.set("Please fill in all fields".to_string());
                    return;
                }

                loading.set(true);
                error.set(String::new());

                spawn_local(async move {
                    match auth::login(email_val, password_val).await {
                        Ok(auth_response) => {
                            auth_state.login(auth_response);
                        }
                        Err(e) => {
                            error.set(e);
                            loading.set(false);
                        }
                    }
                });
            }>
                <label>
                    "Email"
                    <input
                        type="email"
                        prop:value=move || email.get()
                        on:input=move |ev| email.set(event_target_value(&ev))
                        disabled=move || loading.get()
                    />
                </label>
                <label>
                    "Password"
                    <input
                        type="password"
                        prop:value=move || password.get()
                        on:input=move |ev| password.set(event_target_value(&ev))
                        disabled=move || loading.get()
                    />
                </label>
                <button type="submit" disabled=move || loading.get()>
                    {move || if loading.get() { "Logging in..." } else { "Login" }}
                </button>
            </form>
            <p class="error">{move || error.get()}</p>
            <p class="switch-form">
                "Don't have an account? "
                <a href="#" on:click=move |ev| {
                    ev.prevent_default();
                    on_switch_to_register();
                }>"Register"</a>
            </p>
        </div>
    }
}

#[component]
pub fn Register(
    auth_state: AuthState,
    on_switch_to_login: impl Fn() + 'static + Copy,
) -> impl IntoView {
    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let confirm_password = RwSignal::new(String::new());
    let error = RwSignal::new(String::new());
    let loading = RwSignal::new(false);

    view! {
        <div class="register">
            <h2>"Register"</h2>
            <form on:submit=move |ev| {
                ev.prevent_default();
                let email_val = email.get();
                let password_val = password.get();
                let confirm_val = confirm_password.get();

                if email_val.is_empty() || password_val.is_empty() {
                    error.set("Please fill in all fields".to_string());
                    return;
                }

                if password_val != confirm_val {
                    error.set("Passwords do not match".to_string());
                    return;
                }

                if password_val.len() < 6 {
                    error.set("Password must be at least 6 characters".to_string());
                    return;
                }

                loading.set(true);
                error.set(String::new());

                spawn_local(async move {
                    match auth::register(email_val, password_val).await {
                        Ok(auth_response) => {
                            auth_state.login(auth_response);
                        }
                        Err(e) => {
                            error.set(e);
                            loading.set(false);
                        }
                    }
                });
            }>
                <label>
                    "Email"
                    <input
                        type="email"
                        prop:value=move || email.get()
                        on:input=move |ev| email.set(event_target_value(&ev))
                        disabled=move || loading.get()
                    />
                </label>
                <label>
                    "Password"
                    <input
                        type="password"
                        prop:value=move || password.get()
                        on:input=move |ev| password.set(event_target_value(&ev))
                        disabled=move || loading.get()
                    />
                </label>
                <label>
                    "Confirm Password"
                    <input
                        type="password"
                        prop:value=move || confirm_password.get()
                        on:input=move |ev| confirm_password.set(event_target_value(&ev))
                        disabled=move || loading.get()
                    />
                </label>
                <button type="submit" disabled=move || loading.get()>
                    {move || if loading.get() { "Creating account..." } else { "Register" }}
                </button>
            </form>
            <p class="error">{move || error.get()}</p>
            <p class="switch-form">
                "Already have an account? "
                <a href="#" on:click=move |ev| {
                    ev.prevent_default();
                    on_switch_to_login();
                }>"Login"</a>
            </p>
        </div>
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DashboardView {
    Vehicles,
    FuelEntry(i32),
}

#[component]
pub fn Dashboard(auth_state: AuthState) -> impl IntoView {
    let user_id = auth_state.user().map(|u| u.user_id).unwrap_or(0);
    let vehicles = RwSignal::new(Vec::<Vehicle>::new());
    let stations = RwSignal::new(Vec::<FuelStation>::new());
    let loading = RwSignal::new(true);
    let error = RwSignal::new(String::new());
    let show_add_form = RwSignal::new(false);
    let current_view = RwSignal::new(DashboardView::Vehicles);

    let load_data = move || {
        let user_id = user_id;
        spawn_local(async move {
            let vehicles_result = vehicles::list_vehicles(user_id).await;
            let stations_result = fuel_entries::list_fuel_stations().await;

            match (vehicles_result, stations_result) {
                (Ok(v), Ok(s)) => {
                    vehicles.set(v);
                    stations.set(s);
                    loading.set(false);
                }
                (Err(e), _) | (_, Err(e)) => {
                    error.set(e);
                    loading.set(false);
                }
            }
        });
    };

    Effect::new(move |_| {
        load_data();
    });

    view! {
        <div class="dashboard">
            <Show when=move || loading.get()>
                <p>"Loading..."</p>
            </Show>

            <p class="error">{move || error.get()}</p>

            <Show when=move || !loading.get()>
                <Show
                    when=move || matches!(current_view.get(), DashboardView::Vehicles)
                    fallback=move || view! {
                        <button on:click=move |_| current_view.set(DashboardView::Vehicles)>
                            "< Back to Vehicles"
                        </button>
                    }
                >
                    <section class="vehicles-section">
                        <div class="section-header">
                            <h2>"Your Vehicles"</h2>
                            <button on:click=move |_| show_add_form.set(!show_add_form.get())>
                                {move || if show_add_form.get() { "Cancel" } else { "Add Vehicle" }}
                            </button>
                        </div>

                        <Show when=move || show_add_form.get()>
                            <AddVehicleForm
                                user_id=user_id
                                on_success=move || {
                                    show_add_form.set(false);
                                    load_data();
                                }
                            />
                        </Show>

                        <Show when=move || !vehicles.get().is_empty()>
                            <ul class="vehicle-list">
                                <For each=move || vehicles.get() key=|v| v.id let(vehicle)>
                                    <VehicleItem
                                        vehicle=vehicle
                                        on_delete=move || load_data()
                                        on_select=move |id| current_view.set(DashboardView::FuelEntry(id))
                                    />
                                </For>
                            </ul>
                        </Show>

                        <Show when=move || vehicles.get().is_empty()>
                            <p class="empty-state">"No vehicles yet. Add your first vehicle above!"</p>
                        </Show>
                    </section>
                </Show>

                <Show when=move || matches!(current_view.get(), DashboardView::FuelEntry(_))>
                    {move || {
                        if let DashboardView::FuelEntry(vehicle_id) = current_view.get() {
                            if let Some(vehicle) = vehicles.get().iter().find(|v| v.id == vehicle_id) {
                                view! {
                                    <FuelEntrySection
                                        vehicle=vehicle.clone()
                                        stations=stations.get()
                                        on_back=move || current_view.set(DashboardView::Vehicles)
                                    />
                                }.into_any()
                            } else {
                                view! { <p>"Vehicle not found"</p> }.into_any()
                            }
                        } else {
                            view! { <span></span> }.into_any()
                        }
                    }}
                </Show>
            </Show>
        </div>
    }
}

#[component]
pub fn AddVehicleForm(user_id: i32, on_success: impl Fn() + 'static + Copy) -> impl IntoView {
    let make = RwSignal::new(String::new());
    let model = RwSignal::new(String::new());
    let registration = RwSignal::new(String::new());
    let error = RwSignal::new(String::new());
    let loading = RwSignal::new(false);

    view! {
        <form class="add-vehicle-form" on:submit=move |ev| {
            ev.prevent_default();
            let make_val = make.get();
            let model_val = model.get();
            let reg_val = registration.get();

            if make_val.is_empty() || model_val.is_empty() || reg_val.is_empty() {
                error.set("Please fill in all fields".to_string());
                return;
            }

            loading.set(true);
            error.set(String::new());

            spawn_local(async move {
                match vehicles::create_vehicle(make_val, model_val, reg_val, user_id).await {
                    Ok(_) => {
                        make.set(String::new());
                        model.set(String::new());
                        registration.set(String::new());
                        loading.set(false);
                        on_success();
                    }
                    Err(e) => {
                        error.set(e);
                        loading.set(false);
                    }
                }
            });
        }>
            <label>
                "Make"
                <input
                    type="text"
                    placeholder="e.g., Toyota"
                    prop:value=move || make.get()
                    on:input=move |ev| make.set(event_target_value(&ev))
                    disabled=move || loading.get()
                />
            </label>
            <label>
                "Model"
                <input
                    type="text"
                    placeholder="e.g., Corolla"
                    prop:value=move || model.get()
                    on:input=move |ev| model.set(event_target_value(&ev))
                    disabled=move || loading.get()
                />
            </label>
            <label>
                "Registration"
                <input
                    type="text"
                    placeholder="e.g., ABC123"
                    prop:value=move || registration.get()
                    on:input=move |ev| registration.set(event_target_value(&ev))
                    disabled=move || loading.get()
                />
            </label>
            <button type="submit" disabled=move || loading.get()>
                {move || if loading.get() { "Adding..." } else { "Add Vehicle" }}
            </button>
            <p class="error">{move || error.get()}</p>
        </form>
    }
}

#[component]
pub fn VehicleItem(
    vehicle: Vehicle,
    on_delete: impl Fn() + 'static + Copy + Send + Sync,
    on_select: impl Fn(i32) + 'static + Copy + Send + Sync,
) -> impl IntoView {
    let show_confirm = RwSignal::new(false);
    let deleting = RwSignal::new(false);
    let vehicle_id = vehicle.id;

    view! {
        <li class="vehicle-item">
            <div class="vehicle-info" on:click=move |_| on_select(vehicle_id)>
                <strong>{vehicle.make.clone()}" "{vehicle.model.clone()}</strong>
                <span class="registration">{vehicle.registration.clone()}</span>
            </div>
            <div class="vehicle-actions">
                <Show
                    when=move || show_confirm.get()
                    fallback=move || view! {
                        <button class="delete-btn" on:click=move |_| show_confirm.set(true)>
                            "Delete"
                        </button>
                    }
                >
                    <span>"Confirm? "</span>
                    <button
                        on:click=move |_| {
                            deleting.set(true);
                            spawn_local(async move {
                                let _ = vehicles::delete_vehicle(vehicle_id).await;
                                on_delete();
                            });
                        }
                        disabled=move || deleting.get()
                    >
                        {move || if deleting.get() { "Deleting..." } else { "Yes" }}
                    </button>
                    <button on:click=move |_| show_confirm.set(false) disabled=move || deleting.get()>
                        "No"
                    </button>
                </Show>
            </div>
        </li>
    }
}

#[component]
pub fn FuelEntrySection(
    vehicle: Vehicle,
    stations: Vec<FuelStation>,
    on_back: impl Fn() + 'static + Copy,
) -> impl IntoView {
    let entries = RwSignal::new(Vec::<FuelEntry>::new());
    let loading = RwSignal::new(true);
    let error = RwSignal::new(String::new());
    let show_add_form = RwSignal::new(false);
    let vehicle_id = vehicle.id;

    let load_entries = move || {
        let vehicle_id = vehicle_id;
        spawn_local(async move {
            match fuel_entries::list_fuel_entries(vehicle_id).await {
                Ok(e) => {
                    entries.set(e);
                    loading.set(false);
                }
                Err(e) => {
                    error.set(e);
                    loading.set(false);
                }
            }
        });
    };

    Effect::new(move |_| {
        load_entries();
    });

    view! {
        <section class="fuel-entry-section">
            <h2>{vehicle.make}" "{vehicle.model}" ("{vehicle.registration}")"</h2>

            <div class="section-header">
                <h3>"Fuel Entries"</h3>
                <button on:click=move |_| show_add_form.set(!show_add_form.get())>
                    {move || if show_add_form.get() { "Cancel" } else { "Add Entry" }}
                </button>
            </div>

            <Show when=move || show_add_form.get()>
                <AddFuelEntryForm
                    vehicle_id=vehicle.id
                    stations=stations.clone()
                    on_success=move || {
                        show_add_form.set(false);
                        load_entries();
                    }
                />
            </Show>

            <Show when=move || loading.get()>
                <p>"Loading entries..."</p>
            </Show>

            <p class="error">{move || error.get()}</p>

            <Show when=move || !loading.get() && !entries.get().is_empty()>
                <ul class="entry-list">
                    <For each=move || entries.get() key=|e| e.id let(entry)>
                        <FuelEntryItem entry=entry on_delete=move || load_entries() />
                    </For>
                </ul>
            </Show>

            <Show when=move || !loading.get() && entries.get().is_empty()>
                <p class="empty-state">"No fuel entries yet. Add your first entry above!"</p>
            </Show>
        </section>
    }
}

#[component]
pub fn AddFuelEntryForm(
    vehicle_id: i32,
    stations: Vec<FuelStation>,
    on_success: impl Fn() + 'static + Copy,
) -> impl IntoView {
    let station_query = RwSignal::new(String::new());
    let selected_station_id = RwSignal::new(None::<i32>);
    let mileage = RwSignal::new(String::new());
    let litres = RwSignal::new(String::new());
    let cost = RwSignal::new(String::new());
    let error = RwSignal::new(String::new());
    let loading = RwSignal::new(false);
    let show_dropdown = RwSignal::new(false);

    let filtered_stations = Memo::new(move |_| {
        let query = station_query.get().to_lowercase();
        stations
            .iter()
            .filter(|s| s.name.to_lowercase().contains(&query))
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
    });

    view! {
        <form class="add-entry-form" on:submit=move |ev| {
            ev.prevent_default();
            let mileage_val = mileage.get().parse::<i32>();
            let litres_val = litres.get().parse::<f64>();
            let cost_val = cost.get().parse::<f64>();

            match (mileage_val, litres_val, cost_val) {
                (Ok(m), Ok(l), Ok(c)) if l > 0.0 && c > 0.0 => {
                    loading.set(true);
                    error.set(String::new());

                    spawn_local(async move {
                        match fuel_entries::create_fuel_entry(
                            vehicle_id,
                            selected_station_id.get(),
                            m,
                            l,
                            c,
                        ).await {
                            Ok(_) => {
                                mileage.set(String::new());
                                litres.set(String::new());
                                cost.set(String::new());
                                station_query.set(String::new());
                                selected_station_id.set(None);
                                loading.set(false);
                                on_success();
                            }
                            Err(e) => {
                                error.set(e);
                                loading.set(false);
                            }
                        }
                    });
                }
                _ => {
                    error.set("Please enter valid numbers".to_string());
                }
            }
        }>
            <label>
                "Station (optional)"
                <input
                    type="text"
                    placeholder="Type to search or add new"
                    prop:value=move || station_query.get()
                    on:input=move |ev| {
                        station_query.set(event_target_value(&ev));
                        selected_station_id.set(None);
                        show_dropdown.set(true);
                    }
                    on:focus=move |_| show_dropdown.set(true)
                    on:blur=move |_| {
                        set_timeout(move || show_dropdown.set(false), std::time::Duration::from_millis(200));
                    }
                    disabled=move || loading.get()
                />
                <Show when=move || show_dropdown.get() && !filtered_stations.get().is_empty()>
                    <ul class="autocomplete-dropdown">
                        <For each=move || filtered_stations.get() key=|s| s.id let(station)>
                            <li on:click=move |_| {
                                station_query.set(station.name.clone());
                                selected_station_id.set(Some(station.id));
                                show_dropdown.set(false);
                            }>
                                {station.name.clone()}
                            </li>
                        </For>
                    </ul>
                </Show>
            </label>
            <label>
                "Mileage (km)"
                <input
                    type="number"
                    placeholder="Current odometer reading"
                    prop:value=move || mileage.get()
                    on:input=move |ev| mileage.set(event_target_value(&ev))
                    disabled=move || loading.get()
                />
            </label>
            <label>
                "Litres"
                <input
                    type="number"
                    step="0.01"
                    placeholder="Amount filled"
                    prop:value=move || litres.get()
                    on:input=move |ev| litres.set(event_target_value(&ev))
                    disabled=move || loading.get()
                />
            </label>
            <label>
                "Cost"
                <input
                    type="number"
                    step="0.01"
                    placeholder="Total cost"
                    prop:value=move || cost.get()
                    on:input=move |ev| cost.set(event_target_value(&ev))
                    disabled=move || loading.get()
                />
            </label>
            <button type="submit" disabled=move || loading.get()>
                {move || if loading.get() { "Saving..." } else { "Add Entry" }}
            </button>
            <p class="error">{move || error.get()}</p>
        </form>
    }
}

#[component]
pub fn FuelEntryItem(
    entry: FuelEntry,
    on_delete: impl Fn() + 'static + Copy + Send + Sync,
) -> impl IntoView {
    let show_confirm = RwSignal::new(false);
    let deleting = RwSignal::new(false);
    let entry_id = entry.id;

    view! {
        <li class="entry-item">
            <div class="entry-info">
                <span class="date">{entry.filled_at.clone()}</span>
                <span class="mileage">{entry.mileage_km}" km"</span>
                <span class="litres">{entry.litres}" L"</span>
                <span class="cost">"$"{entry.cost}</span>
                {move || {
                    if let Some(name) = entry.station_name.clone() {
                        view! { <span class="station">{name}</span> }.into_any()
                    } else {
                        view! { <span></span> }.into_any()
                    }
                }}
            </div>
            <div class="entry-actions">
                <Show
                    when=move || show_confirm.get()
                    fallback=move || view! {
                        <button class="delete-btn" on:click=move |_| show_confirm.set(true)>
                            "Delete"
                        </button>
                    }
                >
                    <span>"Confirm? "</span>
                    <button
                        on:click=move |_| {
                            deleting.set(true);
                            spawn_local(async move {
                                let _ = fuel_entries::delete_fuel_entry(entry_id).await;
                                on_delete();
                            });
                        }
                        disabled=move || deleting.get()
                    >
                        {move || if deleting.get() { "Deleting..." } else { "Yes" }}
                    </button>
                    <button on:click=move |_| show_confirm.set(false) disabled=move || deleting.get()>
                        "No"
                    </button>
                </Show>
            </div>
        </li>
    }
}

pub fn main() {
    mount_to_body(App);
}
