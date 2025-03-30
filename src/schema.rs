// @generated automatically by Diesel CLI.

diesel::table! {
    assets (id) {
        id -> Int4,
        asset_path -> Varchar,
        user_id -> Int4,
        public -> Bool,
    }
}

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
        pid -> Uuid,
        is_admin -> Bool,
        permission_group -> Int2,
        #[max_length = 200]
        root_folder -> Nullable<Varchar>,
        folder_max_size -> Nullable<Int8>,
        created_at -> Timestamp,
    }
}

diesel::joinable!(assets -> users (user_id));
diesel::joinable!(user_permissions -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    assets,
    user_permissions,
    users,
);
