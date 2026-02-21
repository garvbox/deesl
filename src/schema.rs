// @generated automatically by Diesel CLI.

diesel::table! {
    fuel_entries (id) {
        id -> Int4,
        vehicle_id -> Int4,
        station_id -> Nullable<Int4>,
        mileage_km -> Int4,
        litres -> Float8,
        cost -> Float8,
        filled_at -> Timestamp,
        created_at -> Timestamp,
    }
}

diesel::table! {
    fuel_stations (id) {
        id -> Int4,
        name -> Text,
        created_at -> Timestamp,
    }
}

diesel::table! {
    users (id) {
        id -> Int4,
        email -> Text,
        password_hash -> Text,
        created_at -> Timestamp,
    }
}

diesel::table! {
    vehicle_shares (id) {
        id -> Int4,
        vehicle_id -> Int4,
        shared_with_user_id -> Int4,
        permission_level -> Text,
        created_at -> Timestamp,
    }
}

diesel::table! {
    vehicles (id) {
        id -> Int4,
        make -> Text,
        model -> Text,
        registration -> Text,
        created -> Timestamp,
        owner_id -> Int4,
    }
}

diesel::joinable!(fuel_entries -> fuel_stations (station_id));
diesel::joinable!(fuel_entries -> vehicles (vehicle_id));
diesel::joinable!(vehicle_shares -> users (shared_with_user_id));
diesel::joinable!(vehicle_shares -> vehicles (vehicle_id));
diesel::joinable!(vehicles -> users (owner_id));

diesel::allow_tables_to_appear_in_same_query!(
    fuel_entries,
    fuel_stations,
    users,
    vehicle_shares,
    vehicles,
);
