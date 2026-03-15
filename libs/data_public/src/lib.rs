#![no_std]

#[macro_export]
macro_rules! struct_field_public {
    (
        $($(#[$struct_meta:meta])*
        $vis:vis struct $name:ident $(<
            $($generic:ident $(: $bound:tt $(+ $more_bounds:tt)* )? ),* $(,)?
        >)? {
            $(
                $(#[$field_meta:meta])*
                $_vis:vis $field:ident: $ty:ty
            ),* $(,)?
        })*
    ) => {
        $($(#[$struct_meta])*
        $vis struct $name $(<
            $($generic $(: $bound $(+ $more_bounds)* )? ),*
        >)? {
            $(
                $(#[$field_meta])*
                pub $field: $ty
            ),*
        })*
    };
}

#[macro_export]
macro_rules! tuple_field_public {
    (
        $(#[$struct_meta:meta])*
        $vis:vis struct $name:ident $(<
            $($generic:ident $(: $bound:tt $(+ $more_bounds:tt)* )? ),* $(,)?
        >)? (
            $(
                $(#[$field_meta:meta])*
                $ty:ty
            ),* $(,)?);
    ) => {
        $(#[$struct_meta])*
        $vis struct $name $(<
            $($generic $(: $bound $(+ $more_bounds)* )? ),*
        >)? ($($($field_meta)* pub $ty,)*);
    };
}

#[macro_export]
macro_rules! func_public {
    () => {};
}

#[macro_export]
macro_rules! impl_func_public {
    () => {};
}
