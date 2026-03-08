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
        user_id -> Nullable<Int4>,
    }
}

diesel::table! {
    temp_imports (id) {
        id -> Uuid,
        user_id -> Int4,
        vehicle_id -> Int4,
        csv_data -> Bytea,
        created_at -> Timestamp,
    }
}

diesel::table! {
    users (id) {
        id -> Int4,
        email -> Text,
        password_hash -> Nullable<Text>,
        created_at -> Timestamp,
        currency -> Text,
        google_id -> Nullable<Text>,
        distance_unit -> Text,
        volume_unit -> Text,
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
diesel::joinable!(fuel_stations -> users (user_id));
diesel::joinable!(temp_imports -> users (user_id));
diesel::joinable!(temp_imports -> vehicles (vehicle_id));
diesel::joinable!(vehicle_shares -> users (shared_with_user_id));
diesel::joinable!(vehicle_shares -> vehicles (vehicle_id));
diesel::joinable!(vehicles -> users (owner_id));

diesel::allow_tables_to_appear_in_same_query!(
    fuel_entries,
    fuel_stations,
    temp_imports,
    users,
    vehicle_shares,
    vehicles,
);
