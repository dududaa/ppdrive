// @generated automatically by Diesel CLI.

diesel::table! {
    users (id) {
        id -> Int4,
        is_admin -> Bool,
        permission_group -> Int2,
        created_at -> Timestamp,
    }
}
