// @generated automatically by Diesel CLI.

diesel::table! {
    vehicles (id) {
        id -> Int4,
        make -> Text,
        model -> Text,
        registration -> Text,
        created -> Timestamp,
    }
}
