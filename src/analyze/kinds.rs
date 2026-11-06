use paste::paste;

macro_rules! kind {
    ($ident:ident, $test_ident:ident, $kind:expr, $named:literal, $kind_id:literal) => {
        pub const $ident: u16 = $kind_id;

        #[test]
        fn $test_ident() {
            let l = crate::analyze::language();
            let actual_kind_id = l.id_for_node_kind($kind, $named);
            assert_eq!(
                $kind_id, actual_kind_id,
                "node id for {:?} is {} (not {})",
                $kind, actual_kind_id, $kind_id,
            );
        }
    };
}

macro_rules! kinds {
    (
        $kind:ident = $kind_id:literal;
        $($rest:tt)*
    ) => {
        paste! {
            kind!([<$kind:upper _KIND>], [<test_ $kind>], stringify!($kind), true, $kind_id);
        }

        kinds!($($rest)*);
    };
    (
        $kind:literal ($ident:ident) = $kind_id:literal;
        $($rest:tt)*
    ) => {
        paste! {
            kind!([<$ident:upper _KIND>], [<test_ $ident>], $kind, false, $kind_id);
        }

        kinds!($($rest)*);
    };
    () => {};
}

kinds! {
    identifier = 1;
    "[" (left_bracket) = 70;
    "static" (static_keyword) = 71;
    "]" (right_bracket) = 72;
    "=" (equals) = 73;
    preproc_def = 165;
    preproc_function_def = 166;
    preproc_params = 167;
    function_definition = 196;
    declaration = 198;
    type_definition = 199;
    function_declarator = 230;
    enum_specifier = 247;
    struct_specifier = 249;
    union_specifier = 250;
    field_declaration = 253;
    enumerator = 256;
    parameter_declaration = 260;
    field_identifier = 360;
    type_identifier = 362;
}
