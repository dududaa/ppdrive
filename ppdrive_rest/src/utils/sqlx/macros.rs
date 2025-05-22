#[macro_export]
macro_rules! sqlx_binder {
    ($conn:expr, $query:expr, $( $const_value:expr ),*; $variables:expr) => {
        let mut query = sqlx::query($query)
            $(.bind($const_value))*;

        for var in $variables {
            query = query.bind(var);
        }

        query.execute(&$conn)
            .await?;
    };
}
