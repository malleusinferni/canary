extern crate canary;

macro_rules! generate {
    ( $name:ident ) => {
        #[test]
        fn $name() {
            let path = format!("tests/{}.cy", stringify!($name));
            let _ = canary::compile(path).unwrap();
        }
    }
}

generate!(while_loops);
generate!(arrays);
generate!(records);
generate!(functions);
