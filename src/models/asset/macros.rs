#[macro_export]
macro_rules! asset_batch_save {
    ($conn:expr, $query:expr, $( $const_value:expr ),*; $paths:expr) => {
        let mut query = sqlx::query($query)
            $(.bind($const_value))*;

        for path in $paths {
            query = query.bind(path);
        }

        query.execute(&$conn)
            .await?;
    };
}
