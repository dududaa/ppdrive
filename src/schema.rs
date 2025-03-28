// @generated automatically by Diesel CLI.

diesel::table! {
    user_permissions (id) {
        id -> Int4,
        user_id -> Int4,
        permission -> Int2,
    }
}

diesel::table! {
    users (id) {
        id -> Int4,
        is_admin -> Bool,
        permission_group -> Int2,
        created_at -> Timestamp,
    }
}

diesel::joinable!(user_permissions -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    user_permissions,
    users,
);
